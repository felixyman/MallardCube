# Plan 021: Retire the two retail stub fallbacks through converter-owned SQL generation

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/convert_tabular.rs generated_retail_analytics/ data/retailanalytics_tabular src/engine/model.rs src/execute/dispatch.rs src/tools/qualify.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/015-converted-measure-execution-tests.md`, `plans/016-placeholder-sql-contract.md`, `plans/019-conservative-fallback-capability.md`
- **Category**: direction
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

`generated_retail_analytics` is the repo’s second real converted proof model,
but it still qualifies `BLOCKED` because `Gross Profit` and `Total COGS` are
intentional TODO fallback stubs.

Retiring those two stubs is the shortest path to turning retail from a partial
converter demo into a meaningful converted-model proof artifact.

## Current state

- `generated_retail_analytics/conversion-report.md` now says `simple: 2,
  sql_fallback: 2, manual: 0`.
- The only remaining blocked measures are:
  - `Gross Profit`
  - `Total COGS`
- Both fallback files are still TODO stubs.

Relevant excerpts:

```sql
-- generated_retail_analytics/sql_fallback/gross_profit.sql
-- TODO: Implement DuckDB SQL equivalent.
SELECT 1 AS dummy;
```

```sql
-- generated_retail_analytics/sql_fallback/total_cogs.sql
-- TODO: Implement DuckDB SQL equivalent.
SELECT 1 AS dummy;
```

```rust
// src/tools/convert_tabular.rs
// these shapes are already classified as `sql_fallback`
```

```md
// plans/README.md reconcile status
generated_retail_analytics -> BLOCKED (2 retail fallback measures still intentionally stubbed)
```

Repo conventions to match:

- Drive converter improvements from the checked-in retail source export.
- Prefer narrow, model-proven SQL generation over broad speculative DAX support.
- Keep converted artifacts reproducible from source.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Re-convert retail | `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/retail-stub-retire` | exit 0 |
| Qualify retail | `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` | no longer blocked by these two stubs |
| Retail tests | `cargo test --lib retail_analytics_` | all pass |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/tools/convert_tabular.rs`
- `generated_retail_analytics/`
- retail focused tests and qualification expectations

**Out of scope**:
- broader generated-project work
- generalized arbitrary DAX engine support
- security-role handling

## Steps

### Step 1: Characterize the exact retail measure patterns from source

Work from `data/retailanalytics_tabular`, not from hand-edited output. The two
target patterns are:

- `Total COGS`: joined `SUMX(FILTER(...), qty * RELATED(unit_cost))`
- `Gross Profit`: arithmetic composition of other measures

**Verify**: re-convert to `/tmp/opencode/retail-stub-retire` and confirm only these two remain in fallback before the new lowering work.

### Step 2: Add narrow SQL generation for the two patterns

Implement the smallest converter-owned lowering that is honest for this retail
model:

- real SQL fallback for `Total COGS`
- real SQL fallback or safe composed SQL for `Gross Profit`

Keep capabilities conservative (most likely scalar-only unless grouped support is
explicitly proved).

### Step 3: Regenerate and re-qualify `generated_retail_analytics/`

Regenerate the checked-in retail artifact from source once the converter output
is correct.

**Verify**: `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` -> no longer blocked by `Gross Profit` / `Total COGS` stubs.

### Step 4: Lock it with value assertions

Add or update retail execution tests so these measures are no longer covered
only as fail-closed stubs.

**Verify**: `cargo test --lib retail_analytics_` -> all pass.

## Test plan

- Reuse plan 015 execution-path tests and extend them to the retired retail measures.
- Add config/report regression checks so the retail artifact does not drift back
  to TODO stubs.

## Done criteria

- [ ] `cargo test --lib` exits 0
- [ ] `generated_retail_analytics` no longer qualifies `BLOCKED` because of `Gross Profit` / `Total COGS` stubs
- [ ] The checked-in retail artifact is regenerated from converter-owned output
- [ ] Retail execution tests assert values or honest scalar-only behavior for the retired measures
- [ ] `plans/README.md` status row updated

## STOP conditions

- Correct SQL for either measure requires broader semantics than the retail export exposes.
- Grouped Excel behavior cannot be supported honestly under the current fallback contract.

## Maintenance notes

- Keep the scope anchored to the two checked-in retail blockers.
- If one of the two measures still cannot be supported honestly, retire only the other and document the remaining block explicitly.
