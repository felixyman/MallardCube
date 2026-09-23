# Plan 051 — Scale hardening: one engine, bounded responses, request budgets

## Status

- **Priority**: P1 (the proxy must never OOM or hang when data grows)
- **Effort**: L
- **Risk**: MEDIUM (engine topology change, response streaming)
- **Depends on**: 050 (measurements and fixtures)
- **Related**: 054 (engine settings surface, protocol-overhead gate)
- **Category**: performance / robustness

## Why this matters

Plan 050 measured three ways the proxy fails *by growing*, not by breaking:

- the pool opens `pool_size()` (16 by default) **independent DuckDB instances**
  on the same file (`BackendPool::open`, `src/backend/mod.rs:135`), each with its
  own buffer pool and DuckDB's per-instance memory ceiling — the server-wide
  ceiling is 16× that, unenforced;
- responses are built as one `String` (`xmla/discover/members.rs`,
  `execute/render.rs`): a 200k-member hierarchy listing is 156–237 MB and grows
  the process 65 MB → 625 MB; 1M members is ~1 GB per request;
- nothing bounds a request: no timeout, no cell/member cap, no byte cap. Excel
  sends `<Timeout>0</Timeout>` (no timeout) and the proxy has no `Timeout`
  handling at all, so a pathological query pins a connection forever.

The goal is not "fast at any cost": it is **bounded, predictable behaviour** —
constant memory per response, explicit errors instead of OOM kills, and limits
that are visible in `/status`.

## Design

### A. One engine, many connections

Today `open_read_only(path)` is called per pooled slot, i.e. N `Database`
instances. Replace with one `duckdb::Database` per source plus N `Connection`s
from it: one buffer pool, one catalog, one temp directory, one place to set
limits. (DuckDB is the only engine today; the shape matters because a remote
engine in the same slot must not change the pool's contract — plan 054.)

- Limits come from the **engine settings surface (plan 054-C, landed
  2026-09-23 in `src/engine/settings.rs`)**: a cgroup-aware memory ceiling (not
  a share of host RAM — on Kubernetes the pod limit is the truth), a spill
  directory, and the thread count. `MALLARDCUBE_*` environment variables
  override the computed defaults and `/status` reports the effective values.
- `preserve_insertion_order=false` is **not** switched on yet: it can change
  result order where the SQL has no `ORDER BY`, so it lands with the oracle
  corpus and a bench run that prove the Excel-visible shapes are unchanged.
- `threads`: keep the engine default (all cores) but make the pool size and the
  query semaphore (below) the concurrency control; document the trade-off.
- The aggregation sidecar `ATTACH` becomes instance-wide (attach once, not per
  connection) — verify against the plan 041 reload path.
- `/status` reports the effective settings and their source
  (`cgroup | host | env | default`), plus pool size, in-flight and queued
  queries.
- Fallback if a shared `Database` fights the read-only/RLS test suite: keep the
  per-connection instances but divide the ceiling by the pool size and set the
  spill directory on each, and record why in this plan.

*Increment 1 (2026-09-23): the settings surface and the `/status` block landed.
The shared-`Database` change turned out to be **unavailable**: duckdb-rs
1.10503.1 exposes no `Database` type or `open_from`, so multiple connections
cannot share one engine instance. The substitute now implemented is the
plan 051-B semaphore plus a ceiling divided by the slot count — that is what
makes the bound real. Revisit only if the binding grows a shared-handle API.*

### B. Backpressure and budgets

*Increment 1 landed 2026-09-23 (semaphore + timeout + memory division).*

- A global `tokio::sync::Semaphore` sized by
  `MALLARDCUBE_MAX_CONCURRENT_QUERIES` (default: cores / 4, at least 1) is
  acquired by **every** XMLA request and held for its duration, so the bound is
  exact: slots × per-connection ceiling ≤ budget. A request that waited more
  than 50 ms logs it. Measured at 50M rows, concurrency 8: p90 1933 → 1312 ms,
  p99 3020 → 2043 ms, max 3219 → 2336 ms, throughput slightly up, discover
  957 → 1382 req/s; p50 +55 ms (requests queue instead of oversubscribing).
- `MALLARDCUBE_QUERY_TIMEOUT_S` (default 300, `0` disables): the handler wraps
  the blocking worker in `tokio::time::timeout` and, on expiry, calls the
  connection's `InterruptHandle` — the same checkout the worker is running on —
  then answers `Query exceeded the N second timeout and was cancelled`.
  Verified on the 200k-member fixture: a 1.2 s request answers a fault at
  1.01 s, and the connection serves the next request normally.
- The memory ceiling's cgroup default is divided by the slot count, which is
  what makes it a real bound (see 054-C); DuckDB's Rust binding exposes no
  shared `Database`, so the per-slot division is the substitute for one
  process-wide engine.
- Response budgets also landed (commit `93e2137`): member (`> 1M`), cell
  (`> 2M`), and whole-response byte (`> 512 MB`) caps, each tunable to 0 =
  off, each answering with a fault that names the limit and the env var.
  Verified on the 200k-member fixture: a 1000-member cap faults the 200k
  request and passes a 5-member one; a 10-cell cap faults a 21-cell drilldown
  and passes a 5-cell one; a 1 MB byte cap faults a 237 MB response with its
  size.
- Still open in this section: the result cache's byte accounting (64 entries,
  each able to hold a large `QueryResult`).
- Response budgets, checked before and during rendering:
  `MALLARDCUBE_MAX_MEMBERS_PER_RESPONSE`, `MAX_CELLS`, `MAX_RESPONSE_BYTES`.
  On breach: a SOAP fault naming the limit (the shape Excel renders as a data
  source error), never a hang and never an OOM. The reference engine behaves
  the same way in principle ("query cancelled"); confirm the fault shape
  against the mirror before shipping.
- The result cache (`src/execute/cache.rs`) is bounded by *entries* (64) but an
  entry can be a 200k-row `QueryResult`. Add byte/row accounting so capacity is
  a memory budget, and expose entries/bytes/hit-rate in `/status`.

### C. Streaming responses

*Increment 1 landed 2026-09-23 (member rows, single-pass composition).*

Member rows are now held as **data** (no pre-rendered XML per row) and the
response is composed in one pass through `soap_envelope_parts()` +
`discover_rowset_parts()` — no joined payload, no second envelope copy.
Measured on the 200k-member fixture, release build, same request, body
byte-identical to before:

| | pre-change | after |
|---|---|---|
| latency | 1.68 s | **1.02 s** |
| peak RSS | 1,230 MB | **469 MB** |

*Increment 2 landed 2026-09-23 (true streaming).* `MemberRowset` implements
`Stream` and the handler returns `Body::from_stream`: chunks of
`STREAM_CHUNK_ROWS` (2,000) rows are rendered, written to the socket, and
dropped. The XMLA trace takes a bounded preview instead of the full payload, so
a wide rowset no longer writes hundreds of megabytes into the trace file, and
the byte budget is checked against a pre-computed estimate before the first
chunk.

Same request, release build, each stage measured against the previous one:

| | latency | peak RSS |
|---|---|---|
| before the refactor | 1.68 s | 1,230 MB |
| rows as data, single pass | 1.02 s | 469 MB |
| streamed chunks | **0.95 s (TTFB 322 ms)** | **305 MB** |

The body is byte-identical throughout (sha256 of the `<soap:Body>` part), the
transfer uses `chunked` encoding, and Excel resolves `CUBESETCOUNT`/`CUBEVALUE`
over it.

### C-cells. Cell rendering: measured, and the cliff was not the XML

Before building XML streaming for cellsets (plan 034's original scope), a
deliberately large pivot was measured on the wide fixture: Full Date (4,018
rows) x Category (21 columns) = 84,399 cells, 16 MB of response.

| | before | after |
|---|---|---|
| total | 13.51 s | **1.44 s** |
| engine (`sql_execute_us`) | 0.78 s | 0.81 s |
| render (`xml_render_us`) | 12.77 s | **0.59 s** |

The render time was two O(cells x rows) loops, not XML copies:

- `build_cross_tab` resolved each cell with `rows.iter().find(..)` and summed
  every matching row (`collect::<Vec<f64>>()`) for each (All) cell, and called
  `measure_ids_for` per cell. Now: one pass builds an exact-coordinate map plus
  per-dimension and grand totals; the cell loop is O(cells). The oracle tests
  caught a double-counted grand total in the first attempt.
- `build_multi_dim_pivot`'s `distinct` deduplicated with a linear scan per row
  (`values.iter().any(..)`), O(rows x distinct). Now a `HashSet`, and exact
  coordinates resolve through a map built once.

Memory was a red herring: the process peaked at 1.38 GB while the response was
16 MB — but DuckDB alone peaks at 1.14 GB for the same group-by (measured with
the CLI), so the engine's scan dominates and `memory_limit`/`temp_directory`
(051-A/054-C) are the knobs, not XML streaming (which would save ~16-32 MB
here). Excel renders the result correctly: a Category x Channel pivot against
the demo model shows the Grand Total 521,586,767.

Still open in this section:

1. **A cross-joined pair on Columns is cross-wired (mirror-measured
   2026-09-23).** Statement, run against both engines with the same model:

   ```mdx
   SELECT NON EMPTY Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}) ON ROWS,
          NON EMPTY CrossJoin(
            Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)}),
            Hierarchize({DrilldownLevel({[Channel].[Channel].[All]},,,INCLUDE_CALC_MEMBERS)})) ON COLUMNS
   FROM [Model|Sales] WHERE ([Measures].[Revenue]) CELL PROPERTIES VALUE, FORMAT_STRING
   ```

   | | mirror (SSAS 2025) | proxy |
   |---|---|---|
   | Axis0 (columns) | 45 tuples = 9 x 5, each `(Category, Channel)`, `(All)` first and the inner (Channel) member varying fastest | 8 tuples labelled `[Category]...` carrying **date keys** (`.&[2020]`, `.&[2021]`) |
   | Axis1 (rows) | 8 tuples = Date (`All` + 7 years) | 5 tuples labelled `[Date].[Calendar]...` carrying **channel keys** (`.&[Direct]`, `.&[Online]`) |
   | SlicerAxis | — | measures + Territory/Segment `(All)` |
   | cells | 360 = 45 x 8 | 8 |

   It is not a dropped dimension but a **dimension/value cross-wiring**: each
   edge gets the other's count and keys, and most cells are missing. Nothing
   errors, and a report built on it looks plausible while being wrong. The
   Full Date variant of the same shape is worse still: 4,019 of ~0.5 M cells
   and 10.9 s of rendering (the wildcard lookups below).

   **Fixed 2026-09-23.** Root cause: the plan's group-by columns follow
   `query.axis_dimensions` (statement appearance order), while the renderer
   derived key positions from the *specs'* order — the two agree only when the
   statement lists COLUMNS first. Key columns are now resolved **by dimension
   name**, both edges build their coordinates and tuples the same way
   (`edge_coords`/`edge_tuples`, no more "axis 0 is one dimension"), cell
   coordinates are assembled in the plan's key order (`cell_coord`), and
   hierarchy lists follow the edge. `oracle_crossjoined_pair_on_columns_matches_the_reference`
   pins the mirror's shape (105 tuples here, `(All)` first, inner member
   fastest, rows unchanged, every existing combination present, and the
   grand-total/wholesale/Automotive cell values). Excel renders the gesture —
   two fields in Columns, one in Rows — with Grand Total 521,586,767.

   **Corpus matrix expanded 2026-09-23** (the arrangement space, not just each
   shape in isolation). Mirror-captured and pinned with oracle tests: measures
   cross-joined on **Rows** (21 `(Category, Revenue)` tuples, measure second,
   columns unchanged) and **two fields on both edges** (105 `(Category,
   Channel)` row tuples, 8 `(Date, Revenue)` column tuples, grid of cells).
   Clause order is covered by the differential test above. A third case came
   out of the same sweep: a measure on an axis *and* in the slicer, which the
   reference rejects — "The Measures hierarchy already appears in the Axis1
   axis." — while the proxy rendered it silently; the check now lives in
   `unsupported_features` (AST level) so both entry points fault identically.
   Excel cannot drive the Values-on-Rows arrangement through COM (the field
   refuses to move for OLAP pivots), so the mirror-measured oracle test is its
   evidence.

   **Two cheap follow-ups landed 2026-09-23:** `axis_dimension_ids` now returns
   dimensions in axis order (COLUMNS first, then ROWS) rather than clause
   order, which removes the disagreement class by construction — it also fixed
   the same latent bug in the two-field cross-tab renderer, which assigned the
   plan's first two columns to its axes positionally. Key lookups that miss now
   trip a `debug_assert!` and log. A differential test pins the property that
   the whole bug family violated: the same statement written ROWS-first and
   COLUMNS-first must produce byte-identical cellsets (verified to fail with
   the normalization reverted).

   **Design implication** (see plan 049's phase 4): this was a disagreement
   between two representations of the same structure — a flat
   `axis_dimensions` list for SQL and per-axis `AxisSpec`s for rendering —
   with no invariant tying them together. One axis descriptor built from the
   AST, carrying explicit key-column indices and consumed by both the emitter
   and the renderer, removes the whole class; a length check at the
   plan/execute boundary would have turned this silent wrongness into a loud
   failure. The corpus also needs the *matrix* (fields x edge x nesting x
   measures placement x **clause order**), not each shape in isolation — the
   clause order was the untested axis that bit us.
2. **Wildcard (All) lookups in the multi-dimension renderer** are still an
   O(rows) scan per cell, which is what makes a large multi-dimension layout
   slow even when the shape is right. The mask-rollup treatment applied to the
   exact coordinates would finish it.
2. **Cell XML streaming** stays available but is not the current bottleneck:
   with the loops fixed, the cell cap bounds the response and the engine
   bounds the memory.

Both must keep the XML byte-identical for the oracle tests; leave true
DuckDB-cursor streaming out of scope.

### D. Build only what the restrictions ask for

*Landed 2026-09-23.* The three member-row builders take the request's
restrictions and skip a dimension before any dictionary or XML work; the
leveled builder skips individual levels too. Measured on the 200k-member
fixture, a `[Category].[Category]` request went from 934 ms cold / 724 ms warm
to 72 ms cold / **1 ms warm** (identical 21-row, 27 KB response), while the
unrestricted 200k-member listing is unchanged. A regression guard asserts an
unmatched restriction issues zero engine queries (`test_support::counting`).

`get_members_response_with_backend` currently builds every dimension's rows and
filters afterwards (plan 050). Pass the restrictions into the builders so a
`HIERARCHY_UNIQUE_NAME=[Category].[Category]` request never enumerates a
200k-member date hierarchy — this is most of the wide-model win even before
streaming lands.

## Scope

**In:** shared engine + limits + semaphore + timeout + budgets + streaming of
cells/member rows + result-cache accounting + `/status` fields.

**Out:** distributed execution, DuckDB-cursor streaming, MDX feature growth,
write paths, changes to rollup design (plan 052) or intake (plan 053).

## Done criteria

- 200k-member listing: bounded peak RSS (< 200 MB), p95 < 1.5 s, byte-identical
  XML to today.
- 50M bench with rollups is not worse (execute p95 ≤ 400 ms at c=8, throughput
  ≥ 80 req/s); c=32 stress keeps RSS under a documented ceiling.
- A query exceeding the timeout or a budget returns an XMLA fault within the
  limit + 1 s; Excel shows the error and stays responsive.
- `/status` exposes the effective engine settings and their source (plan 054-C),
  in-flight/queued queries, cache bytes, and the configured budgets; README/docs
  updated with the new env vars.
- All 516+ lib tests, the oracle corpus, and `proxy-smoke.sh` stay green.

## STOP conditions

- If streaming disturbs cellset structure (member order, `CellOrdinal`, tuple
  shapes) in any oracle test, stop and split: ship member streaming alone.
- If the shared `Database` requires a self-referential lifetime that spreads
  through the codebase, take the per-connection fallback above rather than a
  wide refactor.
