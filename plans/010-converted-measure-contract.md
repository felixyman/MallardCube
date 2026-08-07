# Plan 010: Make converted complex measures behave like real Excel measures

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat a1b1bd4..HEAD -- src/engine/model.rs src/engine/plan.rs src/engine/sql.rs src/bin/convert_tabular.rs generated_project/sql_fallback/ generated_project/conversion-report.md src/project/project.rs README.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: `plans/008-converter-time-metadata.md`, `plans/009-generated-project-compatibility-gate.md`
- **Category**: direction
- **Planned at**: commit `a1b1bd4`, 2026-06-16

## Why this matters

The remaining blocker between "converted model loads" and "teams can really use
this from Excel" is converted complex measures. Today many of them still run as
raw fallback SQL with incomplete shape guarantees, missing slicer integration,
or TODO placeholders.

This plan defines and enforces a real contract for converted complex measures:
which shapes they support, how they inherit slicers, how grouped results are
returned, and when the proxy must fail closed instead of pretending a measure is
usable.

## Current state

- `src/engine/model.rs` — classifies fallback SQL only as `ScalarOnly`, `Full`, or `Stub`.
- `src/engine/plan.rs` — executes `sql_fallback_sql` verbatim once the fallback passes the coarse shape gate.
- `generated_project/conversion-report.md` — 11 measures still land on fallback SQL.
- `generated_project/sql_fallback/*.sql` — current examples include scalar-only SQL, cumulative grouped SQL, and TODO stubs.

Relevant excerpts:

```rust
// src/engine/model.rs:21-29
pub enum FallbackShape {
    ScalarOnly,
    Full,
    Stub,
}
```

```rust
// src/engine/plan.rs:296-304
let fallback_sql = match plan {
    QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => {
        model.meas_def(measure).sql_fallback_sql.as_deref()
    }
    _ => None,
};
let sql = fallback_sql
    .map(|s| s.to_string())
    .unwrap_or_else(|| sql_for_query_plan(model, plan));
```

```sql
-- generated_project/sql_fallback/median_beställning_till_undersökning.sql
SELECT MEDIAN(beställning_till_undersökningsstart) FROM dw_fys_f_undersökning;
```

```sql
-- generated_project/sql_fallback/antal_utförda_remisser_(ack_månad)_cy.sql
SELECT
  c.månad,
  c.år,
  SUM(base_count) OVER (...) AS ack_value
FROM (... COUNT(DISTINCT f.remissnummer) AS base_count ...)
```

```md
// generated_project/conversion-report.md:60-74
11 SQL fallback measures remain, including cumulative CY/CY-1 and median measures.
```

Repo conventions to match:

- Fail closed on unsupported semantics rather than silently guessing.
- Keep direct SQL execution authoritative.
- Preserve Excel-visible captions; do not rename user-facing measures casually.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build | `cargo build --lib` | exit 0 |
| Full tests | `cargo test --lib` | all pass |
| Generated-project tests | `cargo test --lib generated_project` | all generated-project tests pass |
| Replay | `cargo run --bin trace_replay -- xmla-trace.jsonl --project generated_project/proxy-config.json` | exit 0 after this plan |

## Scope

**In scope**:
- `src/engine/model.rs`
- `src/engine/plan.rs`
- `src/engine/sql.rs`
- `src/bin/convert_tabular.rs`
- `src/project/project.rs`
- generated fallback SQL fixtures under `generated_project/sql_fallback/`
- `generated_project/conversion-report.md`
- generated-project tests and replay assertions
- `README.md`

**Out of scope**:
- generic Malloy-runtime escaping/productization work
- full date-hierarchy browsing
- unrelated metadata rowset work

## Steps

### Step 1: Define a stronger fallback-measure capability contract

Replace the coarse `ScalarOnly | Full | Stub` gate with a contract that can
distinguish at least:

- scalar-only totals
- grouped-by-one-dimension
- grouped-by-specific declared dimensions
- unsupported / placeholder

The contract must be explicit in model metadata, not inferred only from ad hoc
string searches.

**Verify**: `cargo build --lib` -> exit 0.

### Step 2: Make converter output declare fallback capabilities honestly

Update the converter so fallback measures carry the shape metadata from Step 1.
Supported grouped cumulative measures should declare the exact dimensions they
support; scalar median-style measures should say scalar-only; TODO stubs should
stay blocked.

Do not label a fallback as fully general unless the SQL really matches the
runtime's requested shape contract.

**Verify**: converter/regression tests pass.

### Step 3: Preserve base-measure semantics in generated cumulative SQL

Fix the generated cumulative/fallback SQL so it derives from the referenced base
measure semantics instead of hardcoded surrogate aggregations when possible.

Use the checked-in example where the base measure `Antal utförda remisser`
already carries `undersökningsstatus = 'UTFÖRD'` semantics, but the generated
cumulative fallback currently counts distinct `remissnummer` directly.

**Verify**: add focused tests and confirm the generated SQL/report reflect the base measure semantics.

### Step 4: Enforce the contract at execution time

Update the planner/executor so fallback measures:

- run only for supported plan shapes
- inherit slicers only when the declared contract says they can
- return `QueryResult::Empty` or an explicit blocked path for unsupported shapes
  rather than producing misleading results

**Verify**: `cargo test --lib generated_project` -> pass.

### Step 5: Prove one converted complex measure through replay

Pick one realistic converted complex measure (preferably a supported cumulative
time measure after Plan 008) and drive it through the generated-project replay
gate from Plan 009.

The goal is one end-to-end proof that a converted complex measure behaves like a
real PivotTable measure under its declared supported shape.

**Verify**: the replay command passes with that measure in the fixture.

## Test plan

- Model/executor tests for the stronger fallback capability enum/metadata.
- Converter tests proving emitted capability metadata.
- Generated-project tests proving unsupported shapes fail closed.
- One replay-backed generated-project assertion for a supported converted complex measure.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] Converted fallback measures declare capability more precisely than `ScalarOnly | Full | Stub`
- [ ] At least one generated-project complex measure is replay-proven under its supported shape
- [ ] Unsupported fallback shapes fail closed
- [ ] `plans/README.md` status row updated

## STOP conditions

- The correct contract requires a much richer `QueryPlan` than the current IR can express.
- Converter-generated SQL cannot preserve referenced base-measure semantics without a separate semantic compiler.
- The chosen proof measure cannot be validated deterministically against synthetic generated data.

## Maintenance notes

- Reviewers should be suspicious of any fallback marked "fully supported".
- Keep the contract explicit and conservative; the product cost of a blocked
  measure is lower than the cost of a wrong PivotTable result.
