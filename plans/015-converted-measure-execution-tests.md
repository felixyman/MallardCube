# Plan 015: Add end-to-end value assertions for converted measures

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/execute/dispatch.rs src/execute/render.rs src/tools/qualify.rs src/backend/mod.rs generated_retail_analytics/proxy-config.json generated_project/proxy-config.json data/generated.db data/seed_generated.sql`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: —
- **Category**: test coverage
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The repo now has real converted proof artifacts, but their tests still mostly
prove envelopes, non-panics, or direct DuckDB values rather than actual proxy
execution results. That leaves room for converted-measure regressions to ship
with a green suite.

Before changing converted measure contracts, add characterization tests that
assert actual values through the execution/render path the proxy exposes.

## Current state

- `src/execute/dispatch.rs` already has converted-project tests, but they are
  shallow:
  - retail `Total Revenue` only checks XML scaffolding.
  - retail stub measures only check that the proxy does not panic.
  - generated-project fallback checks query DuckDB directly instead of going
    through proxy execution.
- `src/backend/mod.rs` still turns invalid scalar SQL into `0.0`, so envelope-
  only tests cannot detect placeholder or broken converted SQL.

Relevant excerpts:

```rust
// src/execute/dispatch.rs:1368-1375
fn retail_analytics_execute_total_revenue_renders_cellset() {
    let xml = get_execute_statement_response(...);
    assert!(xml.contains("mddataset"));
    assert!(xml.contains("<Axes>"));
    assert!(xml.contains("<CellData>"));
}
```

```rust
// src/execute/dispatch.rs:1377-1383
fn retail_analytics_stub_measures_return_empty() {
    let xml = get_execute_statement_response(...);
    assert!(!xml.is_empty(), "should not panic on stub measure");
}
```

```rust
// src/execute/dispatch.rs:1394-1427
let conn = duckdb::Connection::open("data/generated.db")...;
// validates generated-project fallback SQL directly in DuckDB
```

```rust
// src/backend/mod.rs:415-418
pub fn query_scalar(&self, sql: &str) -> f64 {
    conn.query_row(sql, [], |r| r.get::<_, f64>(0)).unwrap_or(0.0)
}
```

Repo conventions to match:

- Excel-safety claims must be backed by execution-path evidence, not just raw DB
  smoke tests.
- Keep the tests narrowly targeted at the checked-in converted projects.
- Prefer additive test scaffolding over broad runtime refactors.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Retail tests | `cargo test --lib retail_analytics_` | all retail tests pass |
| Generated-project tests | `cargo test --lib generated_project` | all generated-project tests pass |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/execute/dispatch.rs`
- small test-only helpers in execution/render/backend code if required to prove
  converted values honestly
- generated retail / generated project test coverage

**Out of scope**:
- changing converted measure semantics
- docs-only cleanup
- converter logic changes except tiny test scaffolding support

## Steps

### Step 1: Decide the minimum honest execution harness for converted projects

Use the smallest addition that lets tests assert real values without relying on
 the global backend singleton or raw DuckDB-only queries. Prefer a test-only
 helper that exercises the planning -> execution -> render path with a specific
 project/config and a file-backed DB path.

Do not start by adding more direct `duckdb::Connection` assertions.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Add retail converted-measure value assertions

Upgrade the current retail tests so they assert values, not only XML presence.
At minimum:

- `Total Revenue` returns the expected scalar value on the current fixture.
- `Gross Margin %` either returns the expected scalar or is explicitly covered
  as blocked/unsupported once plan 016 lands.
- `Gross Profit` / `Total COGS` continue to fail closed without panicking.

Keep the assertions tied to the checked-in retail artifact, not a hand-built
temporary conversion.

**Verify**: `cargo test --lib retail_analytics_` -> all pass.

### Step 3: Add generated-project converted-measure execution assertions

Replace direct-DuckDB-only proof for the two generated-project fallback measures
with at least one assertion through the proxy execution path:

- supported scalar fallback execution returns a non-empty value
- grouped/unsupported shape fails closed when required by the capability
  contract

If a grouped query is not yet honest under the current capability logic, record
that as the characterization that plan 019 must preserve or tighten.

**Verify**: `cargo test --lib generated_project` -> all pass.

### Step 4: Leave the suite with explicit numeric characterization tests

When done, the suite should fail if:

- a converted measure silently turns into `0.0`
- a placeholder SQL expression changes value shape
- a fallback measure starts claiming grouped support without returning grouped
  values

**Verify**: `cargo test --lib` -> all pass.

## Test plan

- Add value assertions for retail `Total Revenue`.
- Add execution-path assertions for generated-project fallback measures.
- Keep at least one fail-closed assertion for intentionally unsupported/stub
  measures.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] Retail converted measures have value assertions, not just XML envelope checks
- [ ] Generated-project fallback measures are exercised through proxy execution, not only raw DuckDB
- [ ] The suite now detects silent `0.0` / placeholder-SQL regressions in converted measures
- [ ] `plans/README.md` status row updated

## STOP conditions

- The only way to test file-backed converted projects honestly is a large runtime
  refactor beyond test scaffolding.
- Converted-project value assertions are nondeterministic on the checked-in data.
- The current generated fixtures are not stable enough to support numeric
  assertions without first changing product behavior.

## Maintenance notes

- Prefer exact value assertions over substring XML checks whenever feasible.
- If you add test-only execution helpers, keep them obviously scoped to tests.
- These tests are the dependency foundation for plans 016 and 019.
