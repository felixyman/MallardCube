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

   **VM sweep 2026-09-23 (after the shape/descriptor work).** `sweep3` (11
   Excel-built layouts): the proxy's report is byte-identical to the pre-change
   baseline, and differs from the mirror on the same 6 lines as before (two
   filter cases and one COM error text). `sweep2` (13 layouts): exactly one
   structural change — the two-fields-in-Columns layout went 23x9 -> **24x30,
   which is what the mirror returns**, so the descriptor work repaired a real
   Excel layout rather than only the synthetic probe; the other four differing
   lines are date-anchored value drift (the demo data regenerates daily).
   `totals_off` still matches the old mirror (102x8) while the new mirror run
   says 42x8 — a sweep-side flake (client-side subtotal state), not a proxy
   change.

   Pre-existing gaps the sweep reproduced (neither is a regression):
   - a value filter delivered as
     `FROM (SELECT Filter(<members>, <condition>) ON COLUMNS ...)` was
     **ignored** — fixed 2026-09-23: the multi-dimension plan paths dropped
     `set_op` entirely, and now rank/filter the outer key column by its totals
     over the inner dimensions. Sweep3's `nested_value_filter` case went from a
     42x2 grid to 24x2, byte-identical to the mirror's line;
   - the sweep's COM-driven Top-N never reaches the proxy (0 `TopCount` requests
     in 1,148 traced ones) while the mirror filters; capturing the mirror's
     request needs the relay in the sweep path;
   - a Units number format difference in `data2` ("232 966,0" vs the mirror's
     "232 966").

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

## Review-driven hardening (2026-09-23, external review + probes)

A second agent reviewed the tree with live probes rather than code reading.
Four **silent wrong answers** were verified and are fixed; each is pinned by a
parser unit test *and* by `scripts/probe-fidelity.sh`, which went from 6 of 9
probes failing to 9 of 9 passing:

| finding | before | after |
|---|---|---|
| CDATA-wrapped `<Statement>` | 453 B / 0 cells (empty success) | the statement runs |
| unparsable entity in a statement | empty success | `Malformed` fault |
| unparsable entity in a restriction | **unrestricted rowset** (4,247 rows) | `Malformed` fault |
| nested `<restriction>` forms (both) | unrestricted rowset | honoured, same fields as the flat form |
| empty or self-closing `<Statement>` | empty success | empty success (the fault was reverted 2026-09-24: the reference accepts it, and MSOLAP sends it for every session begin) |

Text now accumulates per element and is consumed when that element closes —
which also fixes mixed content, where the last text event used to win. Both
nested restriction forms share one `apply_restriction` with the flat form, so
the paths cannot diverge.

**Security**: authored (fallback) SQL is no longer reachable by a restricted
user — the request faults instead of returning unfiltered rows — and `/status`
reports the auth posture (`configured`, `roles`, `rls_active`, `anonymous`).
Open question for the mirror (needs the Windows tools): whether SSAS denies
tables a role does not list; `effective_table_filter` currently documents
"no entry = full".

**Hygiene**: the result cache shares `Arc<QueryResult>` instead of deep-cloning
per hit; non-finite values emit a blank cell rather than invalid `xsd:double`;
`Timings` dropped the never-assigned `semantic_us`/`sql_emit_us` (they read as
0, i.e. "instant"); backend error logs truncate SQL to 300 chars; `rusqlite`
(and its bundled SQLite) is gone; `[profile.release]` states `lto = "thin"` and
`panic = "unwind"` because the fault-on-panic design depends on unwinding.

Still open from that review, in the order I would take them:

1. **Completeness audit** — every requested set, filter, restriction, measure,
   property and option must be *consumed* or faulted. The four bugs above are
   instances of its absence, and it is the change that prevents the next four
   of that shape. Design: carry an inventory of what the request asked for on
   the semantic query, have the plan and renderer mark each entry consumed, and
   fault naming anything left; debug-assert in tests, fault in production.
2. `<Properties>` (`Format`, `Content`, `AxisFormat`, catalog overrides) are
   ignored: harmless for Excel, wrong for other clients.
3. Session id is extracted by a text scan (`body.find("SessionId=\"")`); one
   escaping serializer for cellsets (the raw `{sv}` interpolation is a footgun).
4. Log/trace hygiene: rotating `xmla-trace.jsonl` (one wide member listing writes
   ~300 MB), log levels and correlation ids, and 66 `println!`s in `main.rs`.

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

## Review round 2 (2026-09-24): holes in round 1's fixes

Fifteen findings from a second review pass; the code-level ones are fixed here,
four remain open.

Fixed:

- **Composite plans bypassed the fallback refusal.** `MultiMeasure`, `TupleSet`
  and `MultiGroupBy` carry measure ids directly and decompose into
  `Total`/`GroupBy` recursively, so the check that only inspected the outermost
  variant let a restricted user reach authored SQL through "two measures on an
  axis". `plan_measures` collects every measure now.
- **`DRILLTHROUGH` reached raw `SELECT *` SQL with no user context** — no role
  predicates, no OLS. A restricted role is refused (applying the predicates is
  open below; the refusal is the honest interim).
- **A DAX-only table permission was counted as a restriction but enforced as
  full access.** It now hides the table for that role (fail closed) and warns at
  startup.
- **`/status` claimed `deny` for an `auth` block with no mechanism** while
  requests were administrators. It reports the enforced mode: `configured` means
  a real mechanism (trusted proxy or OIDC), and `rls_active` requires one.
- ~~**A self-closing `<Statement/>`** returned the empty-success shape; it
  faults like the paired empty form.~~ **Reverted the same day.** MSOLAP sends
  `<Statement/>` for every session begin, so faulting it broke every real
  connection (`E_FAIL`). The reference answers empty, self-closing and
  whitespace-only statements with an empty `ExecuteResponse` (verified against
  SSAS 2025 via the mirror, 2026-09-24); both empty-statement faults are gone
  and `probe-fidelity.sh` now asserts the reference behaviour.
- `log_sql` could panic slicing mid-UTF-8-character on the logging path.
- The plan key omitted `level`/`time_flag`, so two filters with the same text at
  different levels could share a result-cache entry.
- Harness: the reviewer prompt scopes with `git diff origin/master..HEAD` and
  demands the commit count; permissions deny `proxy-smoke.sh serve` and
  `rls-rollup-ab.sh` (both bind 8080); the wrapper refuses to reuse or kill a
  process it does not own and warns on a stale binary; two new probes
  (self-closing and whitespace-only statements) bring `probe-fidelity.sh` to
  11 cases.

Open, in order:

1. `Count`/`MetaCount` SQL and relationship-backed member discovery still omit
   role predicates — member counts can disclose restricted totals.
2. Cellset member rendering interpolates unescaped dynamic text (`&`, `<`);
   one escaping serializer should own that boundary.
3. The result cache is entry-bounded, not byte-bounded (known, recorded above).
4. Drillthrough should apply role predicates and OLS rather than refuse.

## VM validation (2026-09-24)

The VM session that followed the review fixes caught a **regression the review
itself had asked for**: faulting an empty `<Statement>` broke every real MSOLAP
connection (`E_FAIL`). The reference (SSAS 2025 via the mirror) answers empty,
self-closing and whitespace-only statements with an empty `ExecuteResponse`,
and MSOLAP sends `<Statement/>` for every session begin. Both faults are
reverted (commit `5737101`); `probe-fidelity.sh` now asserts the reference
behaviour for all three shapes.

With that fixed, `sweep3` through Excel is **byte-identical to the pre-review
baseline** on the proxy, and the mirror run is byte-identical to its own
baseline. The two engines differ in exactly one data line:

- **Top-5 pivot filter — resolved (2026-09-25)**: Excel sends the filter
  server-side as a **subselect**, identical to both engines:

  ```mdx
  SELECT NON EMPTY Hierarchize({DrilldownLevel({[Category].[Category].[All]},,,INCLUDE_CALC_MEMBERS)})
    DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME ON COLUMNS
  FROM (SELECT Generate(Hierarchize({[Category].[Category].[All]}) AS [XL_Filter_Set_0],
          BottomSum(Except(DrilldownLevel([XL_Filter_Set_0].Current AS [XL_Filter_HelperSet_0], , 0, INCLUDE_CALC_MEMBERS),
            [XL_Filter_HelperSet_0]), 5, [Measures].[Revenue])) ON COLUMNS
    FROM [Model] WHERE ([Measures].[Revenue]))
  WHERE ([Measures].[Revenue]) CELL PROPERTIES ...
  ```

  Captured through the logging relay (8095→8090) while Excel applied a Top-5
  pivot filter: the reference evaluates the subselect and answers Axis0 =
  `{All, Toys}` — `BottomSum` means "the bottom members whose cumulative sum
  reaches 5", which is the single lowest-revenue category, so Excel's grid is
  `Toys | 24 440 800` + Grand Total (the sweep baseline's 3x2). The proxy
  ignores the `FROM (SELECT ...)` clause and answers all 21 members (the 22x2
  grid), and Excel re-sent the filtered query once — the same
  unexpected-response retry seen with ADODB. There was no `TopCount` because
  there is none to send: the filter is the subselect.

  So this is not a query-shape mystery but the `NON EMPTY`/subselect gap
  already recorded for `axis-date-key-drilldown`; parity needs subselect
  evaluation (`Generate`, `BottomSum`, `Except`, `DrilldownLevel`, `Current`,
  `Alias`), or a narrow recognizer for Excel's filter idiom that computes the
  same cumulative set from SQL.

Two environment notes for the next VM session:

- The 8090 mirror is a C# `HttpListener` (`pump-proxy2.ps1`) forwarding to
  `https://localhost:8443`; it does not survive a reboot and must be started
  before any mirror run.
- A stale WinINET proxy (`127.0.0.1:8888`, with `<-loopback>`) makes
  MSOLAP/Excel fail to connect while `curl` keeps working — clear
  `HKCU\...\Internet Settings\ProxyEnable` first.
- A cold Excel connection rejects automation calls (`RPC_E_CALL_REJECTED`) for
  a few seconds; the sweeps need a short settle after `Refresh()`.

### MDSCHEMA_MEMBERS gaps (fixed)

Measured against the mirror: a request restricted to `[Date].[Full Date]`
returned **0** members where SSAS returns 4019, and `[Measures]` returned 0
where SSAS returns 6 (`[Category].[Category]` matched at 21). The key attribute
hierarchy was advertised in `MDSCHEMA_HIERARCHIES` (plan 048) but never
enumerated, so Excel's date-filter member list came back empty. Fixed in
`216fe45`: both hierarchies list their members now (proxy 4019 / 6 / 21 vs
mirror 4019 / 6 / 21), in the same namespace conventions the axis uses.
`sweep3` through Excel is unchanged by the fix (the only line that moved is a
flaky COM error message on a spec that fails against both engines).

Remaining parity nit: the mirror formats date member names/captions as locale
short dates (`1/1/2020`) where the proxy emits the raw ISO value
(`2020-01-01`), and its unique names carry `T00:00:00`. Same open item as the
compound-member naming in the skill notes.

### ADODB loop (diagnosed 2026-09-25)

`ADODB.Connection.Execute` sends `<Format>Tabular</Format>` in the Execute
properties. The reference answers that with a **flattened rowset**
(`urn:schemas-microsoft-com:xml-analysis:rowset`, one `<row>` per result row);
the proxy ignores the property and answers a cellset. MSOLAP cannot consume it
as a recordset, so it re-sends the query for every field read — the
"7,776-request loop" is one HTTP Execute per read.

Reproduced and pinned on the VM, with both engines behind logging relays:

- the same ADODB probe (one Execute, then 5,000 field reads) costs the mirror
  **6 requests** total and the proxy **2,418** (identical 7,390 B cached
  cellset each, until the watchdog killed it);
- trigger test against the reference: `<Format>Tabular</Format>` alone ⇒
  rowset; `<Content>SchemaData</Content>` alone ⇒ cellset; both ⇒ rowset;
  neither ⇒ cellset;
- rowset shapes: `SELECT [Measures].[Revenue] ON 0 FROM [Model]` ⇒ one column
  (`[Measures].[Revenue]`, element name escaped `_x005B_Measures_x005D_._x005B_Revenue_x005D_`),
  one row, value `5.21586767E8`; with `[Category].[Category].Members` on the
  other axis ⇒ 21 rows, columns
  `[Category].[Category].[Category].[MEMBER_CAPTION]` + `[Measures].[Revenue]`;
- Excel never asks this way: all 11 `Format=Tabular` hits in the 1,156-request
  corpus are `DiscoverLiterals`, and every Excel Execute uses `Format=Native` —
  which is why the Excel sweeps never noticed.

Fix scope: honour `Format=Tabular` on Execute by rendering the same SQL plan
output as the rowset shape (escaped column names per the reference's scheme,
role suffix `MEMBER_CAPTION` for axis members, doubles in the reference's
`G9`-style form, one row per group-by tuple), then add the ADODB probe as a
regression (a bounded request count for the 5,000-read script). Probe two
dimensions, multiple measures and the `(All)` row before implementing.

## Review round 3 (2026-09-24, VM session)

The third review verified the empty-statement revert (no production path faults
an empty statement; the reference agrees) and that **all seven hierarchies'
`MDSCHEMA_MEMBERS` counts match the mirror** (6 / 21 / 5 / 4206 / 4019 / 6 / 9).
It found one live silent-wrong-answer and one stale literal, both fixed in
`fbc4661`:

- `MEMBER_TYPE` was parsed-and-dropped: `MEMBER_TYPE=1` returned 21 rows where
  the mirror returns 20, and `MEMBER_TYPE=2` returned 21 where the mirror
  returns 1. It is parsed and applied now (verified live: 20 / 1).
- `[Measures]` hardcoded `DEFAULT_MEMBER=[Measures].[Total Sales]` — another
  project's measure — and `HIERARCHY_ORIGIN=2` where the mirror reports 6. The
  default member is the model's first measure now; the mirror's
  `[Measures].[__Default measure]` placeholder is a deliberate divergence. The
  date key attribute hierarchy keeps origin 2 (verified against Excel in plan
  048).

Open, in priority order (recorded, not fixed):

1. **A DAX-only table permission hides its table but leaves other tables
   full.** `effective_table_filter` returns `Hidden` for the table carrying the
   unlowerable filter, but a role with no entry for the fact table still gets
   `Full` there, so a measure over the fact runs unfiltered. Whether SSAS
   propagates the dimension filter through the relationship (and so restricts
   the fact aggregate) is **unverified** — the next mirror experiment: create a
   role with a filter on a dimension, connect with `Roles=`, compare a fact
   total. Either way the honest options are to lower the filter or refuse the
   query, not to hide one table and serve the rest.
2. **`Count`/`MetaCount` omit role predicates**, and relationship-backed member
   discovery uses the dimension table's own access only — a fact-table role
   filter does not reach the member dictionary. Axis `DISPLAY_INFO` and child
   counts read the unfiltered cache too, so the leak is wider than
   `MDSCHEMA_MEMBERS`.
3. **Most metadata rowsets ignore the user**: dimensions, hierarchies, levels,
   measures, tables, measure-group dimensions and properties iterate the full
   model without `effective_table_filter`, so a hidden table is still
   advertised.
4. **`CATALOG_NAME`/`CUBE_NAME` restrictions are parsed but never checked** — a
   request naming another catalog or cube still gets the configured one's rows,
   and `MDSCHEMA_MEASURES` carries no restrictions at all. Same "restriction
   silently dropped" class as `MEMBER_TYPE`.
5. Known: cache is entry-bounded, not byte-bounded; member rows materialize
   before streaming; cellsets still need one escaping serializer.
6. The ADODB `Execute` retry loop remains undiagnosed; response shape, session
   handling and transport headers are the remaining candidates.

### RLS bucket: measured semantics, and what is fixed

Measured on the reference with probe roles:

- unlisted tables stay visible (a role listing two tables still sees six
  dimensions) — our union semantics are correct;
- a fact filter does **not** restrict dimension members (Territory stays at 9);
- a dimension filter **does** propagate to fact totals (65,850,256 of
  521,586,767) — so hiding a table whose DAX filter cannot be lowered leaks;
- the filtered table's own members shrink (Territory 9 → 2).

Fixed: `Count`/`MetaCount` role predicates (`e2a1b64`); refusal for unlowerable
DAX filters (`d984495`); OLS-hidden metadata across six rowsets plus a
no-model-permission refusal (`3b99c04`).

Still open in this bucket: axis `DISPLAY_INFO`/child counts read the unfiltered
dimension cache, so a restricted user's member child counts are the
unrestricted ones (an information leak, not row data); and catalog/cube scope
names are still unvalidated (we return the configured catalog's rows where the
reference answers an empty rowset).

### RLS review round 2: what the end-of-bucket review falsified

Fixed in this round:

- `MDSCHEMA_MEMBERS` bypassed the no-permission refusal (it is handled before
  the dispatch-level check), so a role-less user could still read the member
  dictionary; the member builder refuses too now.
- Table permissions matched table names exactly, so `Product` against a
  `product` table silently became full access; matching is case-insensitive.
- A hidden *dimension* was still queryable through axes and filters (the plan
  emitted no deny predicate for it); a plan that references a hidden dimension
  now faults instead of reading its members.
- `Count` checked the fact-table fallback for relationship-backed dimensions
  rather than the counted table; it now uses `dim_table_for_discovery`.
- `[Measures].Members` was misread as a level of the first dimension, so
  `COUNT([Measures].Members)` counted categories (20) instead of measures (6);
  the count path resolves the measure set now. A measure set *on an axis* still
  needs a real, user-aware expansion — open.
- `user_is_restricted`/`unhonourable_filter_fault` now look at *effective*
  access, so a second role granting full access (the documented union
  semantics) no longer causes a false refusal.

Recorded, not fixed (metadata names only, no row data; or behaviour agreed as
out of scope):

- `MDSCHEMA_PROPERTIES`, `MDSCHEMA_MEASUREGROUPS`, the `[Measures]` part of
  `MDSCHEMA_MEMBERS` and the `TMSCHEMA_*` stubs still ignore OLS visibility.
- Cellset rendering builds the SlicerAxis from every model dimension and
  resolves `STRTO_MEMBER`/member-only probes against the full model.
- Unknown session ids are accepted (documented statelessness) and fault
  responses carry no session header.

### Table-permission case sensitivity — measured on the reference (2026-09-25)

Four roles on MallardDemo (created with TMSL over the pump, probed with
`Roles=` over ADOMD):

| permission name | setting | revenue | `MDSCHEMA_DIMENSIONS` |
|---|---|---|---|
| `Category` | `metadataPermission: none` | 521,586,767 | 6 dims, no `[Category]` |
| `category` | `metadataPermission: none` | 521,586,767 | 6 dims, no `[Category]` |
| `Category` | `filterExpression: 'Category'[Category] = "Toys"` | 24,440,800 | — |
| `category` | `filterExpression: 'Category'[Category] = "Toys"` | 24,440,800 | — |

Both spellings take effect, so the reference matches table names
**case-insensitively**: our `eq_ignore_ascii_case` change matches the engine
rather than diverging (the review's "high, conditional" finding is resolved in
favour of the current behaviour). Roles deleted afterwards; `rls_probe`,
`rls_terr` and `rls_sales` from the earlier measurements remain on the mirror.

### Scope names (fixed 2026-09-25)

Measured on the reference, then implemented:

- a Discover restriction naming another catalog or cube (`CATALOG_NAME`,
  `CUBE_NAME`) answers that rowset's **empty shape** — zero rows, schema
  intact. Dimensions, measures, hierarchies, levels, properties, measure
  groups, measure-group dimensions, tables and cubes carry their restrictions
  now and check the scope before any work;
- a `<Catalog>` **property** naming another database is a **fault**, for
  Discover and Execute alike: "Either the user, '…', does not have access to
  the 'X' database, or the database does not exist." (checked once at dispatch
  through `XmlaRequest::property_catalog`);
- `FROM [NoSuchCube]` faults "The NoSuchCube cube does not exist." — the
  `SemanticQuery` carries the FROM cube now, so the cellset path refuses it
  without parsing the MDX twice;
- both names match case-insensitively.

`parity/catalog.json`: `dimensions-wrong-cube-is-empty` lost its `known_gap`,
and `dimensions-wrong-catalog-is-empty` plus `execute-unknown-cube-faults`
were added — 19/19 matched with two known gaps left (the `DIMENSION_VISIBILITY`
filter and the date-key drilldown).

Still open in this bucket: the OLS leftovers in `MDSCHEMA_PROPERTIES`,
`MDSCHEMA_MEASUREGROUPS`, the `[Measures]` part of `MDSCHEMA_MEMBERS` and the
`TMSCHEMA_*` stubs; the axis `DISPLAY_INFO`/child counts; the result cache's
byte accounting.

### OLS leftovers (fixed 2026-09-25)

- `MDSCHEMA_PROPERTIES`: a hidden dimension's member-property rows disappear,
  and the `[Measures]` rows disappear when no measure's fact table is visible —
  the three builders that computed measure visibility inline now share
  `discover::measures_visible`.
- `MDSCHEMA_MEASUREGROUPS`: a hidden fact table loses its group.
- `MDSCHEMA_MEMBERS`: `[Measures]` lists only visible measures.
- `TMSCHEMA_TABLES` and `TMSCHEMA_RELATIONSHIPS` describe **this model** now —
  fact tables then dimension tables with stable ids, and the model's own
  relationships (resolved through the fact table's `id`, which is what
  `fact_table_id` holds) — and are OLS-filtered. The remaining TMSCHEMA
  rowsets stay empty stubs (`COLUMNS`, `PARTITIONS`), recorded.

Verified live with a scratch auth config: a hidden `date_dim` removes `[Date]`
from MDSCHEMA_PROPERTIES (38 → 22 rows) while `[Measures]` stays; a hidden
`sales_fact` empties MDSCHEMA_MEASUREGROUPS and the `[Measures]` member list;
TMSCHEMA_TABLES shows exactly one of `date_dim` / `sales_fact` per role. The
relationship path is covered by a unit test against the converted Contoso
project, since the demo model has no relationships.

Still open in this bucket: the axis `DISPLAY_INFO`/child counts and the result
cache's byte accounting.
