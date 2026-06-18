# Plan 014: Retire generated_project's remaining TODO fallback stubs

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- generated_project/sql_fallback/ src/tools/convert_tabular.rs src/engine/model.rs src/engine/plan.rs generated_project/conversion-report.md src/execute/dispatch.rs src/project/project.rs README.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/011-converted-project-qualification.md`, `plans/013-converted-bootstrap-assets.md`
- **Category**: direction
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

`generated_project` is the repo’s most customer-shaped converted model. It now
loads, passes the compatibility gate structurally, and has fallback capability
gating so unsupported measures fail closed. But two checked-in fallback files
are still explicit `TODO` stubs.

That means the model is still carrying known dead measures in the highest-value
proof artifact. Retiring those stubs is a better next move than broad new
feature work because it turns the strongest real-model fixture into a more
honest and more complete Excel proof.

## Current state

- `src/tools/convert_tabular.rs` — when no supported fallback SQL generator applies, it emits an annotated stub ending with `SELECT 1 AS dummy;`.
- `src/engine/model.rs` and `src/engine/plan.rs` — now classify and gate fallback SQL via `FallbackCapability`, so stubs fail closed instead of returning misleading grouped/scalar data.
- `generated_project/conversion-report.md` — still reports 11 SQL fallback measures.
- `generated_project/sql_fallback/antal_signerade_dvt_remisser.sql` and `generated_project/sql_fallback/medeltid_undersökningsslut_till_signering_(ej_akut).sql` — are still TODO stubs.

Relevant excerpts:

```rust
// src/tools/convert_tabular.rs:895-917
fn render_fallback_stub(name: &str, dax: &str) -> String {
    ...
    -- TODO: Implement DuckDB SQL equivalent.
    SELECT 1 AS dummy;
}
```

```rust
// src/engine/model.rs:24-35
pub enum FallbackCapability {
    Universal,
    ScalarOnly,
    GroupedSpecific(Vec<DimId>),
    Stub,
}
```

```rust
// src/engine/plan.rs:271-309
Some(FallbackCapability::Stub) => Some(QueryResult::Empty)
Some(FallbackCapability::ScalarOnly) => ...
Some(FallbackCapability::GroupedSpecific(ref dims)) => ...
```

```sql
-- generated_project/sql_fallback/antal_signerade_dvt_remisser.sql:7-10
-- TODO: Implement DuckDB SQL equivalent.
SELECT 1 AS dummy;
```

```sql
-- generated_project/sql_fallback/medeltid_undersökningsslut_till_signering_(ej_akut).sql:8-11
-- TODO: Implement DuckDB SQL equivalent.
SELECT 1 AS dummy;
```

Repo conventions to match:

- Fail closed when semantics are unknown.
- Prefer real SQL on the direct DuckDB path over pretending the runtime can infer missing semantics.
- A converted-project proof artifact should not silently depend on known placeholder measures.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Full tests | `cargo test --lib` | all pass |
| Generated-project tests | `cargo test --lib generated_project` | all generated-project tests pass |
| Compatibility replay | `cargo run --bin xmla_proxy -- trace-replay xmla-trace.jsonl generated_project/proxy-config.json` | exit 0 when replay fixture is applicable |

## Scope

**In scope**:
- `generated_project/sql_fallback/`
- `src/tools/convert_tabular.rs` when a safe generic SQL generator is possible
- `src/engine/model.rs`
- `src/engine/plan.rs`
- generated-project tests / replay assertions
- `generated_project/conversion-report.md`
- `README.md` only if user-facing claims need adjustment

**Out of scope**:
- broad new DAX-family support outside these exact stubbed measures
- hierarchy work
- security-role enforcement

## Steps

### Step 1: Characterize the two remaining stub measures precisely

For each stubbed measure, decide whether the right treatment is:

- a generic converter rule
- a model-specific checked-in SQL fallback
- or an explicit continued block if the semantics cannot be stated safely

Use the existing generated SQL/report/model context; do not guess from measure
names alone.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Replace the stubs with honest executable behavior

Implement the narrowest correct improvement for each measure:

- if the SQL can be generated safely, teach the converter to emit it
- if it cannot yet be generalized, replace the checked-in stub with explicit,
  real SQL and pair it with the correct fallback capability metadata

Do not leave `SELECT 1 AS dummy;` in any measure that the project still exposes
as available.

**Verify**: `grep -R "SELECT 1 AS dummy\|TODO: Implement DuckDB SQL equivalent" generated_project/sql_fallback` -> no matches for the targeted measures.

### Step 3: Prove the measures through generated-project tests

Add focused tests that show the new fallback behavior is honest:

- supported shapes execute and return non-empty results
- unsupported shapes still fail closed when required by the capability contract

Prefer direct generated-project tests; use replay only when the trace fixture
already exercises the measure shape or can be extended deterministically.

**Verify**: `cargo test --lib generated_project` -> pass.

### Step 4: Update the conversion report and project proof claims

Refresh `generated_project/conversion-report.md` so it no longer implies these
two measures are unresolved TODOs. If one remains intentionally blocked, make
that explicit in the report rather than leaving a stub.

**Verify**: `grep -n "TODO\|dummy" generated_project/conversion-report.md generated_project/sql_fallback/*.sql` -> no stale stub language for the retired measures.

## Test plan

- Add generated-project regression tests for both previously stubbed measures.
- Cover both supported execution and fail-closed behavior where applicable.
- Reuse the existing generated-project load/execute tests as the structural pattern.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] The two named generated-project fallback stubs are either replaced with real SQL or explicitly removed from availability
- [ ] Generated-project tests prove the new behavior honestly
- [ ] The generated-project report no longer treats these measures as silent TODO stubs
- [ ] `plans/README.md` status row updated

## STOP conditions

- Implementing either measure correctly requires semantics that are not available in the converted model/data shape.
- The only viable SQL depends on unsupported runtime features outside the direct DuckDB path.
- The available generated-project fixture data cannot prove the new result deterministically.

## Maintenance notes

- Reviewers should inspect the SQL itself, not just whether tests pass.
- Prefer honest blocking over clever-looking but semantically wrong fallbacks.
- If a generic converter rule emerges while implementing one of these measures, keep it tightly scoped and covered by tests.
