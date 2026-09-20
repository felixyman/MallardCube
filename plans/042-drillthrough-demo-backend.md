# Plan 042: Retire the in-memory demo backend (fixes file-backed DRILLTHROUGH)

## Status

- **Priority**: P1 (correctness — silently wrong or empty results)
- **Effort**: M (test seam refactor)
- **Risk**: LOW (compile-time isolation is the guarantee)
- **Depends on**: none
- **Category**: correctness / architecture hygiene
- **Status**: **DONE 2026-09-20**

## Why this mattered

The production DRILLTHROUGH path ("Show Details" / double-click a cell in
Excel) read the **in-memory demo database**, not the project's DuckDB file.
For a file-backed project this returned either no rows (table-name mismatch) or
— worse — synthetic demo rows when the project's fact table happened to be
named `sales_fact` or `fact_table`, the names the demo seed uses.

Verified before the fix against the file-backed Contoso project
(`data/sales.db` has 7,794 rows in `sales`):

```
POST /xmla  <Statement>DRILLTHROUGH</Statement>
→ rows returned: 0
```

Root cause chain:

| Step | Location |
|---|---|
| Production route called the **parameterless** helper | `main.rs` → `execute::dispatch::get_execute_drillthrough_response(mdx)` |
| Helper read the global demo backend | `execute/dispatch.rs` → `Backend::get()` |
| `Backend::get()` lazily created the in-memory demo DB | `backend/mod.rs` — module-level `static BACKEND` + `Backend::new()` seeding `fact_table` + `sales_fact` |
| The server never initialised that static | only `trace_replay` called `init_backend(...)` |
| `Backend::init()` was dead | it set a **function-local** static — the trap that hid all of this |

Every test ran against the demo backend via `with_test_project` +
`Backend::get()`, and all Excel E2E testing used demo-mode project3, so nothing
caught it.

## What was done (2026-09-20)

1. **Deleted the in-memory demo backend and the global static**
   - `Backend::new()` (in-memory, seeded), the module-level `static BACKEND`,
     `instance()`, `init_backend()`, and the dead `Backend::init()` are gone.
   - `BackendSource` (file / demo temp file) is the only backend source;
     production always carries it (`AppState.backend_source` → `checkout()`).
   - The demo *mode* (`db_path: null` → temporary seeded DuckDB file) is
     unchanged and still powers `cargo run`.
2. **Test fixture instead of a production-reachable demo DB**
   - `Backend::test_fixture()` is `#[cfg(test)]` and seeds a temp-file demo DB
     once per test process. The compiler now makes it impossible for production
     code to reach demo data.
3. **DRILLTHROUGH takes the request backend**
   - `get_execute_drillthrough_response(statement, backend)`; `route_request`
     passes the checked-out backend.
4. **Test seams are compiled out of production**
   - `#[cfg(test)]`: `dispatch::get_execute_statement_response`,
     `builders::{execute_semantic_query, get_execute_cellset_response,
     get_execute_mdx_response, get_execute_dax_response}`,
     `render::dispatch`, `members::get_members_response`.
   - Dead benchmark scaffolding removed (`BenchmarkDataConfig`,
     `generate_rows`, `FactRow`, `Backend::new_with_config`) plus the two
     parameterless `execute_plan*` wrappers.
5. **`trace-replay` carries its own backend**
   - Builds a `BackendSource` from the project's `db_path` and replays each
     statement through the runtime (or the drillthrough handler), with an
     admin `UserContext`; no global init.
6. **Docs**: the "in-memory demo database" wording is corrected in `README.md`
   and `docs/DEVELOPER-GUIDE.md` (demo mode has always been a temp *file*).

## Verification

- New regression test `drillthrough_reads_the_backend_it_is_given` runs the
  generated-project fixture (`data/generated.db`, 10 fact rows) and asserts the
  rowset comes from that file.
- End-to-end: Contoso file-backed project → `DRILLTHROUGH` returns 1,000 rows
  (the `LIMIT`) with real columns (`OrderKey`, `OrderDate`, …), not 0.
- 417 tests green, `cargo check --all-targets` clean (no warnings), clippy
  clean, `scripts/proxy-smoke.sh` 8/8.

## Follow-ups (not in this plan)

- **033**: DRILLTHROUGH *filter* semantics — `CAST(col AS VARCHAR) LIKE 'key%'`
  is load-bearing for coarse date members and over-matches flat dimensions;
  needs a level-aware filter.
- `scripts/bench-workload.jsonl`'s captured responses predate the demo-data
  change (11 years → 7), so `trace-replay` now shows data-driven diffs; refresh
  the capture when convenient.
- `execute_plan_sql_with_backend` in `engine/plan.rs` has no callers — candidate
  for deletion in a later cleanup.
