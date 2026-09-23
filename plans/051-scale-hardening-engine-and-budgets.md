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

*Increment 1 (2026-09-23): the settings surface and the `/status` block landed;
the pool still opens one DuckDB instance per connection, so the memory ceiling
is per connection. The shared-`Database` change is the next increment.*

### B. Backpressure and budgets

- A global `tokio::sync::Semaphore` sized by
  `MALLARDCUBE_MAX_CONCURRENT_QUERIES` (default: pool size), acquired in the
  blocking execution path (`main.rs:732` uses `spawn_blocking`), with the
  queue wait recorded in `Timings`. Metadata/Discover requests that are served
  from dictionaries take no permit.
- `MALLARDCUBE_QUERY_TIMEOUT_S`: a watchdog thread calls `Connection::interrupt()`
  (DuckDB has no statement timeout) and the handler returns an XMLA fault.
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

Write cell data and member rows incrementally instead of accumulating one
`String`: `axum::body::Body::from_stream` with a small fixed prefix (schema,
axes, hierarchies) and a closing suffix. Two seams, land them separately:

1. **Member rows** (`xmla/discover/members.rs` → `discover_rowset_envelope`):
   the widest responses by far.
2. **Cell data** (`execute/render.rs`): plan 034's original scope, extended so
   cells and both axis member lists stream.

Both must keep the XML byte-identical for the oracle tests; stream from the
materialized result vectors first (constant extra memory), and leave true
DuckDB-cursor streaming out of scope.

### D. Build only what the restrictions ask for

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
