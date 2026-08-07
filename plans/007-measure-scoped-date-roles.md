# Plan 007: Generalize time intelligence to measure-scoped date roles

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat a1b1bd4..HEAD -- src/project/config.rs src/project/project.rs src/engine/model.rs src/engine/plan.rs src/engine/sql.rs generated_project/proxy-config.json README.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: `plans/006-explicit-time-contract.md`
- **Category**: direction
- **Planned at**: commit `a1b1bd4`, 2026-06-16

## Why this matters

The current time-intelligence design supports one global `date_dim`. That fits
the project3 demo, but it does not fit the repo's own converted-model target:
`generated_project` contains seven separate calendar/date-role relationships.

If the proxy is going to replace real SSAS cubes for Excel users, time-aware
measures must bind to the correct date role (`Order Date`, `Ship Date`,
`Undersökningsslut`, etc.), not to one global calendar chosen at startup.

## Current state

- `src/engine/model.rs` — `SemanticModel` has a single `Option<DateDimDef>`.
- `src/engine/plan.rs` — any time-aware measure injects one filter using that
  single `model.date_dim`.
- `generated_project/proxy-config.json` — already contains many date-role
  relationships but no runtime time-intelligence metadata.
- `docs/ssas-to-malloy-conversion.md` — explicitly calls out role-playing dates.

Relevant excerpts:

```rust
// src/engine/model.rs:160-168
pub struct SemanticModel {
    pub fact_tables: Vec<FactTable>,
    pub dialect: Dialect,
    pub dimensions: Vec<DimensionDef>,
    pub measures: Vec<MeasureDef>,
    pub relationships: Vec<RelationshipDef>,
    pub date_dim: Option<DateDimDef>,
}
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

```json
// generated_project/proxy-config.json:17-45
"relationships": [
  { "fact_column": "remissdatum", "dimension_id": "dw_fys Kalender_Remissdatum", ... },
  { "fact_column": "signeringsdatum", "dimension_id": "dw_fys Kalender_Signeringsdatum", ... },
  { "fact_column": "undersökningsslut", "dimension_id": "dw_fys Kalender_Undersökningsslut", ... }
]
```

```md
// docs/ssas-to-malloy-conversion.md:417-419
DAX time-intelligence functions (`TOTALYTD`, `SAMEPERIODLASTYEAR`, `DATESYTD`, etc.)
do not map to relational queries. Instead, model a date dimension table with
pre-built columns, then filter on those columns in Malloy measures.
```

Repo conventions to match:

- Model identifiers are `String`-backed and loaded from config.
- Measure/fact compatibility is already modeled through `fact_table` and
  relationship metadata; extend that pattern instead of inventing enum-based IDs.
- Keep direct SQL as the authoritative runtime path.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build | `cargo build --lib` | exit 0 |
| Full tests | `cargo test --lib` | all pass |
| Generated-project tests | `cargo test --lib generated_project` | generated-project tests pass |
| Time tests | `cargo test --lib time_intelligence` | all TI tests pass |

## Scope

**In scope**:
- `src/project/config.rs`
- `src/engine/model.rs`
- `src/project/project.rs`
- `src/engine/plan.rs`
- `src/engine/sql.rs`
- `project3/proxy-config.json`
- `generated_project/proxy-config.json`
- related tests in the same files and `src/execute/dispatch.rs`
- `README.md`

**Out of scope**:
- `src/bin/convert_tabular.rs` — handled by Plan 008
- calendar hierarchy browsing
- generic MDX time-function parsing
- any attempt to make Malloy runtime the default path

## Steps

### Step 1: Replace the single-global date-dimension model with role-aware metadata

Refactor the time-intelligence schema so each time-aware measure can name the
date role it uses. The target contract should make it unambiguous which
dimension/relationship a measure binds to.

Prefer evolving the existing schema rather than adding a second one. Example
shape:

- keep shared date-dimension defaults where useful
- add a per-measure date-role binding (for example, `dimension_id` or
  `date_role_dimension_id`) alongside `flag_column`

Update `ProxyConfig`, `MeasureConfig`, `DateDimDef` / replacement model types,
and the project loader accordingly.

**Verify**: `cargo build --lib` -> exit 0.

### Step 2: Thread the selected date role through planning and SQL emission

Update `src/engine/plan.rs` and `src/engine/sql.rs` so time-aware measures:

- inject a synthetic filter for the specific date role named by the measure
- resolve the correct fact-column / relationship / date table
- emit SQL against the bound date-role table, not a global `date_dim`

This plan should preserve current project3 behavior while enabling multiple
date-role measures in the same model.

**Verify**: add focused tests and run `cargo test --lib time_intelligence` -> pass.

### Step 3: Add characterization coverage for multiple date roles

Create tests that prove two measures with identical aggregation but different
date-role bindings produce different SQL / different joins.

Use one small synthetic fixture rather than relying on the full generated
project. The test should confirm the planner can distinguish roles without
hardcoded dimension names.

**Verify**: `cargo test --lib generated_project` or a narrower filter covering the new fixture -> pass.

### Step 4: Apply the new contract to generated_project smoke paths

Update the checked-in `generated_project/proxy-config.json` to exercise the new
schema for at least one real date-role measure. Do not attempt to migrate all
33 measures in this plan; one or two representative measures are enough to
prove the contract.

Add loader assertions that the configured date-role dimension resolves through
the right relationship.

**Verify**: `cargo test --lib generated_project` -> generated-project smoke tests pass.

### Step 5: Update docs to describe the role-aware time model

Update README wording so it no longer implies one global date role. Keep the
statement narrow: role-aware time measures are supported; full calendar
hierarchy browsing remains separate.

**Verify**: `grep -n "role-playing date dimensions" README.md` shows updated wording.

## Test plan

- Config-loader tests for per-measure date-role binding.
- Planner tests proving different measures choose different date roles.
- SQL tests proving the correct relationship/table is used.
- Generated-project smoke assertions for at least one real date-role measure.
- End-to-end execution test for one measure bound to a non-default date role.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] At least one test proves two time-aware measures can bind to different date roles
- [ ] `generated_project` uses the role-aware time schema for at least one measure
- [ ] No single-global-date-dimension assumption remains in the planner path
- [ ] `plans/README.md` status row updated

## STOP conditions

- The refactor requires changing `QueryPlan` variants rather than evolving the
  existing filter model.
- The correct date-role binding cannot be expressed without redesigning
  relationship metadata itself.
- Project3 or generated_project can no longer load with backward-compatible defaults.

## Maintenance notes

- This plan should remove ambiguity, not add two competing time-intelligence
  contracts.
- Reviewers should inspect whether date-role selection is measure-driven all
  the way down to emitted SQL.
- Full calendar hierarchies remain deferred.
