# Plan 016: Stop shipping placeholder SQL as executable converted measures

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/convert_tabular.rs src/backend/mod.rs src/engine/sql.rs generated_retail_analytics/proxy-config.json generated_retail_analytics/conversion-report.md src/execute/dispatch.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/015-converted-measure-execution-tests.md`
- **Category**: correctness
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The converter currently emits placeholder `sql_expr` values like `SUM(1)` and
`SUM(...)` for measures it cannot actually lower honestly. Under the default
direct-SQL runtime, those placeholders are treated as executable SQL contracts.

That is worse than a blocked measure because it can return numerically wrong
results while looking supported.

## Current state

- `render_measure_configs()` emits `sql_expr` for measures classified `simple`.
- `dax_to_sql_hint()` maps a lowered Malloy-like expression through
  `malloy_to_sql()`.
- `malloy_to_sql()` still emits obvious placeholders instead of either real SQL
  or an explicit unsupported contract.
- The checked-in retail project already carries these placeholders.

Relevant excerpts:

```rust
// src/tools/convert_tabular.rs:570-588
let sql_expr = if meas.classification == "simple" {
    dax_to_sql_hint(dax_expr, &meas.classification)
} else {
    "null".to_string()
};
```

```rust
// src/tools/convert_tabular.rs:654-660
fn malloy_to_sql(malloy: &str) -> String {
    if malloy.contains(".sum()") { return "SUM(...)".into(); }
    if malloy.contains(".avg()") { return "AVG(...)".into(); }
    if malloy == "0.8" { return "0.8".into(); }
    "SUM(1)".to_string()
}
```

```json
// generated_retail_analytics/proxy-config.json
"Gross Margin %": { "physical_expr": "gross_profit / total_revenue", "sql_expr": "SUM(1)" }
"Total Revenue": { "sql_expr": "SUM(...)" }
```

```rust
// src/backend/mod.rs:415-418
conn.query_row(sql, [], |r| r.get::<_, f64>(0)).unwrap_or(0.0)
```

Repo conventions to match:

- Fail closed when semantics are unknown.
- Do not overclaim support for converted measures.
- Checked-in converted artifacts should reflect real converter output, not hand
  patches.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Re-convert retail model | `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/retail-contract` | exit 0 |
| Retail tests | `cargo test --lib retail_analytics_` | all pass |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/tools/convert_tabular.rs`
- converted retail artifact regeneration
- small runtime validation if required to reject placeholder SQL contracts
- retail-oriented regression tests

**Out of scope**:
- generalized DAX engine work
- generated-project fallback capability logic (plan 019)
- docs/CLI cleanup (plan 020)

## Steps

### Step 1: Define the placeholder-SQL contract explicitly

Pick one honest rule and apply it consistently:

- either generate real executable SQL for supported simple patterns
- or downgrade unsupported patterns to `sql_fallback` / unavailable instead of
  emitting `SUM(1)` / `SUM(...)` placeholders

Do not keep placeholder SQL strings in emitted configs for measures the runtime
will execute.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Fix the current retail proof artifact through the converter

Use `data/retailanalytics_tabular` as the acceptance fixture and regenerate to a
temp directory until the output is honest.

The expected outcome is:

- supported patterns have concrete SQL
- unsupported patterns are explicit fallbacks or blocked
- no checked-in converted measure is “simple” while still carrying placeholder
  SQL text

**Verify**: `grep -R 'SUM(1)\|SUM(...)\|AVG(...)\|COUNT(...)\|COUNT(DISTINCT ...)' /tmp/opencode/retail-contract/proxy-config.json` -> no placeholder executable SQL for supported measures.

### Step 3: Regenerate `generated_retail_analytics/`

Once the converter output is honest, regenerate the checked-in retail artifact
from source and update any affected tests.

**Verify**: `cargo test --lib retail_analytics_` -> all pass.

### Step 4: Add regression coverage for the contract

Add assertions that a converted “simple” measure must not carry placeholder SQL.
Pair this with the plan 015 value tests so future regressions fail loudly.

**Verify**: `cargo test --lib` -> all pass.

## Test plan

- Add at least one test that inspects emitted converted retail config and fails
  on placeholder executable SQL.
- Keep execution assertions from plan 015 in sync with the new contract.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] No checked-in converted measure is exposed as executable while still using placeholder SQL text
- [ ] `generated_retail_analytics/` is regenerated from converter-owned output
- [ ] Retail execution tests reflect the new honest contract
- [ ] `plans/README.md` status row updated

## STOP conditions

- A measure cannot be downgraded without breaking the repo’s explicit retail proof goal.
- The only path away from placeholder SQL is a broad DAX compiler beyond current scope.
- Regenerated retail output no longer matches the checked-in source export semantics.

## Maintenance notes

- Wrong numbers are worse than blocked measures.
- Prefer explicit downgrade/fallback over clever placeholder SQL.
- Keep the contract check close to the converter so new generated artifacts do not drift.
