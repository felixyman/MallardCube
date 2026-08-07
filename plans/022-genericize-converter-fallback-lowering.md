# Plan 022: Genericize converter fallback SQL lowering

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/convert_tabular.rs generated_retail_analytics/ generated_project/ src/tools/qualify.rs src/execute/dispatch.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/021-retail-stub-retirement.md`
- **Category**: tech-debt / direction
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

Plan 021 retired the two retail stub fallbacks by teaching the converter to
generate real SQL. But the lowering logic in `generate_sql_for_measure()` is
hardcoded to retail-specific measure names and schema details. That means the
retail win is not repeatable — a different model with the same DAX patterns
would still fall through to TODO stubs.

The goal of this plan is to make the converter lowering **pattern-driven, not
project-driven**, so the same DAX families produce real SQL for any converted
model.

## Current state

The converter's fallback SQL dispatch lives in `generate_fallback_sql()` at
`src/tools/convert_tabular.rs:984`. It already has several generic pattern
matchers:

- `generate_calculate_sum()` — generic: parses `CALCULATE(SUM(col), filter)` and
  resolves columns through `resolve_source_column()`.
- `generate_sumx_filter_related()` — generic: parses `SUMX(FILTER(...), col *
  RELATED(dim.col))` and resolves join columns from `model.relationships`.
- `generate_measure_subtraction()` — syntactically generic but depends on
  `generate_sql_for_measure()` for subquery resolution.
- `generate_divide_measure()` — same: depends on `generate_sql_for_measure()`.

The problem is `generate_sql_for_measure()` at line 1384:

```rust
// src/tools/convert_tabular.rs:1384-1408
fn generate_sql_for_measure(name: &str, model: &ConversionModel) -> Option<String> {
    let upper = name.trim().to_uppercase();
    let fact = malloy_name(&model.fact_table.name);

    if upper.contains("TOTAL REVENUE") || upper.contains("REVENUE") {
        Some(format!(
            "SELECT SUM(CAST(net AS DOUBLE)) FROM {fact} WHERE isreturn = 0"
        ))
    } else if upper.contains("TOTAL COGS") || upper.contains("COGS") {
        // ... hardcoded products/productid/unitcost
    } else if upper.contains("GROSS PROFIT") {
        // ... hardcoded net/isreturn/products/productid
    } else {
        None
    }
}
```

This function is called by `generate_measure_subtraction()` and
`generate_divide_measure()` to resolve bracketed measure references like
`[Total Revenue]` into scalar SQL subqueries. The hardcoded name matching means
only retail measures resolve; any other model returns `None` and the entire
pattern falls through to a stub.

Additionally, `src/tools/convert_tabular.rs:1034` has a leftover debug
`eprintln!` in the DIVIDE path.

Repo conventions to match:

- Pattern matchers should resolve columns through `resolve_source_column()`
  and join columns through `model.relationships`, not through hardcoded names.
- The converter should be testable against any Tabular model, not just the
  retail fixture.
- Remove debug prints before committing.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Re-convert retail | `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/retail-022` | exit 0 |
| Qualify retail | `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` | `READY` |
| Retail tests | `cargo test --lib retail_analytics_` | all pass |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/tools/convert_tabular.rs` — `generate_sql_for_measure()` and its callers
- `generated_retail_analytics/` — regenerate from converter after changes
- retail tests in `src/execute/dispatch.rs` if value assertions need updating

**Out of scope**:
- adding new DAX pattern families beyond the existing four
- generated_project fallback changes (those already have real SQL)
- security-role handling
- hierarchy work

## Steps

### Step 1: Replace `generate_sql_for_measure()` with pattern-driven resolution

The function is called by `generate_measure_subtraction()` and
`generate_divide_measure()` to resolve a bracketed measure reference like
`[Total Revenue]` into scalar SQL. Currently it hardcodes retail names.

Replace it with logic that:

1. Finds the measure by name in `model.fact_table.measures` (case-insensitive).
2. If found, calls `generate_fallback_sql()` for that measure's DAX expression.
3. If the generated SQL is a stub (contains `SELECT 1 AS dummy` or `TODO`),
   returns `None`.
4. If not found, returns `None`.

This makes measure-reference resolution recursive and generic: any measure
that can already be lowered by an existing pattern can also be used as a
subquery in arithmetic/division patterns.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Remove the debug `eprintln!`

Delete the line at `src/tools/convert_tabular.rs:1034`:
```rust
eprintln!("DEBUG DIVIDE matched: ...");
```

**Verify**: `grep -n "DEBUG DIVIDE" src/tools/convert_tabular.rs` -> no matches.

### Step 3: Regenerate `generated_retail_analytics/` and verify output is unchanged

Re-convert the retail model and diff the generated SQL fallback files against
the checked-in versions. The output should be identical or functionally
equivalent — the same patterns should produce the same SQL.

If the SQL changes slightly (e.g. column resolution order differs), verify the
new SQL is correct against the retail schema.

**Verify**: `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` -> `READY`.

### Step 4: Add a converter unit test for generic lowering

Add a test that constructs a minimal `ConversionModel` with non-retail table
and column names, then verifies that the same DAX patterns produce real SQL.
This proves the lowering is no longer retail-specific.

At minimum, test:
- `CALCULATE(SUM('MyFact'[Amount]), 'MyFact'[Status] = 1)` produces SQL with
  the correct resolved column names
- `[MeasureA] - [MeasureB]` resolves both measures through the model

**Verify**: `cargo test --lib convert_tabular` or the new test filter -> pass.

### Step 5: Run full suite and verify retail still qualifies READY

**Verify**: `cargo test --lib` -> all pass.
**Verify**: `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` -> `READY`.

## Test plan

- Add a converter unit test with non-retail table/column names to prove
  generic lowering.
- Keep existing retail execution tests as regression coverage.
- Verify retail qualification still returns READY after regeneration.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] `generate_sql_for_measure()` no longer contains hardcoded retail measure
      names (`TOTAL REVENUE`, `TOTAL COGS`, `GROSS PROFIT`) or schema names
      (`net`, `isreturn`, `products`, `productid`, `unitcost`)
- [ ] No debug `eprintln!` in the converter
- [ ] `generated_retail_analytics/` still qualifies `READY` after regeneration
- [ ] A unit test proves non-retail models get real SQL for the same patterns
- [ ] `plans/README.md` status row updated

## STOP conditions

- The recursive `generate_fallback_sql()` call in `generate_sql_for_measure()`
  creates infinite recursion for self-referential measure chains.
- Generic resolution produces different SQL for retail that breaks the
  checked-in execution tests in a way that cannot be fixed without changing
  the test expectations.
- The `ConversionModel` struct does not expose enough metadata to resolve
  columns generically (would require converter parser changes beyond scope).

## Maintenance notes

- After this plan, any new DAX pattern added to `generate_fallback_sql()`
  automatically becomes available as a subquery in arithmetic/division
  patterns — no need to update `generate_sql_for_measure()` separately.
- Reviewers should verify that recursive measure resolution cannot loop.
- If a future model has circular measure references, the resolver should
  return `None` rather than stack-overflowing.
