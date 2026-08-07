# Plan 006: Make the time-intelligence contract explicit and expand period measures

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat a1b1bd4..HEAD -- src/project/config.rs src/project/project.rs src/engine/model.rs src/engine/plan.rs src/engine/sql.rs src/backend/mod.rs data/seed_date_dim.sql project3/proxy-config.json README.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/005-time-intelligence-date-modeling.md`
- **Category**: direction
- **Planned at**: commit `a1b1bd4`, 2026-06-16

## Why this matters

Time intelligence now works for one demo measure (`Revenue YTD`), but the
contract is still implicit and partially default-driven. The current project3
config does not match the runtime schema, the sample cube exposes no explicit
date-role dimension, and the seeded calendar only covers `ytd_flag` and
`prior_year_flag`.

Before adding more ambitious date-role and converted-project work, the repo
needs one explicit, coherent Excel-visible time model that covers the common
period families: YTD, prior year, QTD, and MTD. That turns the current demo
success into a stable contract other plans can safely build on.

## Current state

- `project3/proxy-config.json` — current Excel demo project; now contains the
  `Revenue YTD` measure and a top-level `time_intelligence` block.
- `src/project/config.rs` — deserializes the runtime schema the proxy expects.
- `src/project/project.rs` — builds `SemanticModel.date_dim` and `MeasureDef.time_flag`.
- `src/engine/plan.rs` — injects one synthetic time filter when a measure has a time flag.
- `src/engine/sql.rs` — translates that synthetic filter into a `date_dim` subquery.
- `data/seed_date_dim.sql` — seeded demo calendar table.
- `README.md` — public-facing scope statement; currently says role-playing date dimensions are partial/in progress.

Relevant excerpts:

```json
// project3/proxy-config.json:111-123
"time_intelligence": {
  "date_dim": {
    "table": "date_dim",
    "date_key": "date_key",
    "full_date": "full_date",
    "year": "year",
    "quarter": "quarter",
    "month": "month",
    "flag_columns": {
      "ytd_flag": "ytd_flag",
      "prior_year_flag": "prior_year_flag"
    }
  }
}
```

```rust
// src/project/config.rs:14-39
pub struct TimeIntelligenceConfig {
    pub date_dimension: DateDimensionConfig,
}

pub struct DateDimensionConfig {
    pub dimension_id: String,
    pub date_key_column: String,
    pub full_date_column: String,
    pub table_name: String,
    pub flag_columns: DateFlagColumns,
}
```

```rust
// src/project/project.rs:286-299
let date_dim = config.time_intelligence.as_ref().map(|ti| {
    let dd = &ti.date_dimension;
    let fc = &dd.flag_columns;
    crate::engine::model::DateDimDef {
        dimension_id: dd.dimension_id.clone(),
        table_name: dd.table_name.clone(),
        date_key_column: dd.date_key_column.clone(),
        full_date_column: dd.full_date_column.clone(),
        year_column: fc.year_column.clone(),
        quarter_column: fc.quarter_column.clone(),
        month_column: fc.month_column.clone(),
        ytd_flag_column: fc.ytd_flag_column.clone(),
        prior_year_flag_column: fc.prior_year_flag_column.clone(),
    }
});
```

```rust
// src/engine/plan.rs:102-108
if let Some(date_dim) = &model.date_dim {
    if let Some(flag) = model.meas_def(meas_id).time_flag.as_ref() {
        result.push(TypedDimensionFilter {
            dimension: date_dim.dimension_id.clone(),
            members: vec![],
            time_flag: Some(flag.clone()),
        });
    }
}
```

```sql
-- data/seed_date_dim.sql:11-18
strftime(d, '%Y%m%d')::INTEGER AS date_key,
d::DATE AS full_date,
strftime(d, '%Y')::INTEGER AS year,
CEIL(strftime(d, '%m')::INTEGER / 3.0)::INTEGER AS quarter,
strftime(d, '%m')::INTEGER AS month,
d <= CURRENT_DATE AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y') AS ytd_flag,
... AS prior_year_flag
```

Repo conventions to match:

- Optional config fields use `#[serde(default)]`; see `src/project/config.rs`.
- Project fixture tests live beside loader code in `src/project/project.rs`.
- SQL shape tests live in `src/engine/sql.rs`.
- End-to-end Excel regressions live in `src/execute/dispatch.rs`.
- The public vocabulary is Excel/XMLA first; preserve `catalog`, `cube`,
  `dimension`, `measure`, `date role`, and `time_intelligence` naming.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build | `cargo build --lib` | exit 0 |
| Full tests | `cargo test --lib` | all pass |
| TI tests | `cargo test --lib time_intelligence` | all TI tests pass |
| Project3 tests | `cargo test --lib project3` | project3-related tests pass |

## Scope

**In scope**:
- `src/project/config.rs`
- `src/engine/model.rs`
- `src/project/project.rs`
- `src/engine/plan.rs`
- `src/engine/sql.rs`
- `src/backend/mod.rs`
- `data/seed_date_dim.sql`
- `project3/proxy-config.json`
- `README.md`
- tests in the same files plus `src/execute/dispatch.rs`

**Out of scope**:
- `src/bin/convert_tabular.rs` — handled by Plan 008
- multi-date-role semantics for converted models — handled by Plan 007
- calendar hierarchy browsing — intentionally deferred
- Malloy runtime-specific time support (`MALLOY_RUNTIME=1`)

## Steps

### Step 1: Reconcile the config contract and make project3 explicit

Update `project3/proxy-config.json` to use the schema actually defined in
`src/project/config.rs`:

- Replace the ad hoc `time_intelligence.date_dim.table/date_key/full_date/...`
  block with `time_intelligence.date_dimension.dimension_id/date_key_column/
  full_date_column/table_name/flag_columns`.
- Add a real project3 `Date` dimension entry backed by `date_dim`, mark it as
  `is_date_role: true`, and wire the date-key relationship explicitly so the
  sample model no longer works only through defaults.
- Add a loader test in `src/project/project.rs` that asserts `project3`
  deserializes the explicit date dimension and the `SemanticModel.date_dim`
  fields match the config exactly.

**Verify**: `cargo test --lib project3` -> all project3 tests pass.

### Step 2: Expand the date-flag model to cover common period families

Extend the config/model/calendar contract to cover at least:

- current-year flag
- QTD flag
- MTD flag

Concretely:

- Add the missing fields to `DateFlagColumns` and `DateDimDef`.
- Extend `data/seed_date_dim.sql` to compute those flags using `CURRENT_DATE`.
- Keep the existing YTD and prior-year behavior unchanged.
- Add a backend-level test that proves the seeded table contains true rows for
  each new flag on the current machine date.

Do **not** introduce dynamic MDX parsing for `YTD()` / `MTD()` / `QTD()`.
This plan is still measure-driven, not function-parser-driven.

**Verify**: `cargo test --lib date_dim` -> all date-dimension tests pass.

### Step 3: Add sample project3 measures for the new period flags

Add at least these project3 measures:

- `Revenue Prior Year`
- `Revenue QTD`
- `Revenue MTD`

Each should:

- use the existing `SUM(revenue)` / `revenue.sum()` base expression
- declare the appropriate `time_intelligence.flag_column`
- have its own caption/display name/format string

Add direct loader assertions for these measures in `src/project/project.rs`.

**Verify**: `cargo test --lib project3` -> measure-related assertions pass.

### Step 4: Prove execution, not just SQL text

Add end-to-end tests in `src/execute/dispatch.rs` that execute representative
MDX queries for the new measures and assert:

- the generated plan includes the expected `time_flag`
- the emitted SQL targets the new flag columns
- execution returns a non-empty numeric result against demo data
- measure caption/format metadata in the XML matches the selected measure

Model the structure after the existing `time_intelligence_revenue_ytd_*` test.

**Verify**: `cargo test --lib time_intelligence` -> all TI execution tests pass.

### Step 5: Update public docs to match the explicit contract

Update `README.md` so the documented time-intelligence story matches the live
contract:

- project3 now has an explicit date-role dimension
- the supported demo period measures are YTD / prior year / QTD / MTD
- this remains a flag-based model, not dynamic MDX time-function parsing

Keep the wording Excel-first and avoid promising multi-date-role support yet.

**Verify**: `grep -n "role-playing date dimensions" README.md` shows wording
consistent with the new explicit project3 capability.

## Test plan

- Add/extend config tests in `src/project/config.rs` for the expanded flag-column schema.
- Add loader tests in `src/project/project.rs` for explicit `date_dimension`,
  date-role dimension presence, and new measures.
- Add backend tests in `src/backend/mod.rs` for new seeded flags.
- Add SQL tests in `src/engine/sql.rs` for each new flag family.
- Add end-to-end execution tests in `src/execute/dispatch.rs` for the new measures.
- Use the existing `time_intelligence_revenue_ytd_*` tests as the structural pattern.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] `cargo test --lib time_intelligence` exits 0
- [ ] `project3/proxy-config.json` uses `date_dimension` keys, not the old ad hoc `date_dim` keys
- [ ] project3 contains a real `Date` dimension / relationship contract for time intelligence
- [ ] `plans/README.md` status row updated

## STOP conditions

- The current project3 sample stops loading in Excel before the new tests are green.
- Making the date-role dimension explicit requires changing the public XMLA
  shape in a way that invalidates existing replay fixtures.
- Adding explicit date-role metadata forces Malloy-runtime-only behavior.
- A new flag family cannot be expressed cleanly through date-dimension booleans.

## Maintenance notes

- This plan is the last single-date-dimension cleanup before multi-date-role
  support. Do not invent a second parallel time-intelligence schema.
- Reviewers should scrutinize backward compatibility for existing configs that
  relied on `#[serde(default)]` fallback behavior.
- Dynamic MDX time functions remain deferred; keep the implementation
  measure-driven.
