# Plan 004: Graduate generated_project to a minimal Excel smoke path

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a93b239..HEAD -- src/xmla/discover/members.rs src/engine/model.rs src/engine/plan.rs src/project/project.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: 003 (the cube-agnostic parser from plan 003 is needed
  for the generated project to parse its own cube name)
- **Category**: direction
- **Planned at**: commit `a93b239`, 2026-06-15

## Why this matters

`project3` is a synthetic demo. `generated_project/` contains a converted
real-world SSAS Tabular model: 17 dimensions, 16 relationships, 33 measures.
The proxy already loads it, but at least two paths silently fail because
they don't handle relationship-backed dimensions and don't gate unsupported
fallback measures. This plan makes a narrow, well-defined slice of
`generated_project` actually work end-to-end with Excel, proving the proxy
isn't just "demo-good" — and it surfaces exactly what parts of the
conversion pipeline need attention before a real customer model can ship.

## Current state

- `src/xmla/discover/members.rs:55,85` — member discovery uses `model.dim_table()`
  for both `distinct_count_in` and `distinct_values_in`:
  ```rust
  let cardinality = Backend::get().distinct_count_in(model.dim_table(&dim.id), &dim.physical_field);
  // ...
  let values = Backend::get().distinct_values_in(model.dim_table(&dim.id), &dim.physical_field);
  ```
  This is wrong for relationship-backed dimensions in `generated_project`.

- `src/engine/model.rs:180-185` — `dim_table()` falls back to the primary
  fact table when `DimensionDef.table_name` is `None`:
  ```rust
  pub fn dim_table(&self, dim_id: &str) -> &str {
      let dim = self.dim_def(dim_id);
      dim.table_name.as_deref().unwrap_or(self.primary_table_name())
  }
  ```
  But `table_name` is `None` for any dimension not explicitly bound to a
  `fact_table` — regardless of whether it's actually backed by a
  relationship that points to a different dimension table.

- `src/engine/model.rs:204-206` — `rel_for_dimension()` exists and finds
  the relationship for a dimension, but nothing in metadata paths calls it:
  ```rust
  pub fn rel_for_dimension(&self, dim_id: &str) -> Option<&RelationshipDef> {
      self.relationships.iter().find(|r| r.dimension_id == dim_id)
  }
  ```

- `src/project/project.rs:204-215` — when building the model from config,
  `table_name` is only set from `dimension.fact_table`, never from
  `relationships`:
  ```rust
  let table_name = dc.fact_table.as_ref().map(|ft_id| {
      fact_tables.iter().find(|ft| ft.id == *ft_id)
          .unwrap_or_else(|| panic!(...))
          .table_name.clone()
  });
  ```

- `src/engine/plan.rs:243-251` — fallback SQL is used unconditionally for
  any `Total` or `GroupBy` plan, regardless of whether the fallback SQL is
  a scalar-only `SELECT MEDIAN(...)` that can't handle grouping or a TODO
  stub returning `SELECT 1 AS dummy`:
  ```rust
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
  For `generated_project`, this means clicking a fallback-backed measure in
  Excel either returns a MEDIAN scalar for every cell (if it's a group-by)
  or returns `1` for every cell (if it's a TODO stub).

- `generated_project/` overview:
  - `proxy-config.json` — config with catalog `SEMANTICMODEL`, cube
    `DW_FYS_F_UNDERSÖKNING`, 16 relationships, 17 dimensions, 33 measures
    (22 simple Malloy, 11 `sql_fallback`)
  - `model.malloy` — Malloy source with one fact source and many dimension
    sources
  - `schema.sql`, `sql_fallback/*.sql` — DDL and fallback SQL files
  - `conversion-report.md` — comprehensive conversion report; confirms
    all tables must be manually loaded

Repo conventions and commands: same as prior plans.

## Commands you will need

| Purpose | Command                          | Expected on success            |
|---------|----------------------------------|--------------------------------|
| Build   | `cargo build --lib`              | exit 0, no errors              |
| Tests   | `cargo test --lib`               | all pass (200+ at time of plan)|
| Focused | `cargo test --lib generated_`    | new generated-project tests    |
| Focused | `cargo test --lib project::`     | all project tests pass         |

## Scope

**In scope** (the only files you should modify):
- `src/engine/model.rs` — add `dim_table_for_discovery()` (relationship-aware
  table resolution) and a helper to detect unsupported fallback measure shapes
- `src/xmla/discover/members.rs` — use the relationship-aware table resolver
  instead of `dim_table()`
- `src/project/project.rs` — add generated-project load tests only.
  Do NOT populate `DimensionDef.table_name` from relationships;
  use the new `dim_table_for_discovery()` method for metadata paths instead.
- `src/engine/plan.rs` — gate fallback SQL by shape compatibility; reject
  grouped use of scalar-only fallbacks, reject TODO stubs

**Out of scope** (do NOT touch):
- `generated_project/` files — do not modify the generated artifacts; this
  plan works with what the converter already produces
- `src/execute/render.rs`, `src/execute/axis_members.rs` — unchanged
- `src/mdx/` — unchanged (plan 003 handles parser hardening)
- Any attempt to make `generated_project` runnable as a default — this plan
  only adds tests that load and validate it, plus fixes metadata paths
- Writing fallback SQL for TODO stubs — that's a data-work task, not code

## Git workflow

- Branch: `advisor/004-generated-project-smoke`
- Commit per step.
- Commit message style: "fix: ..." or "feat: ..."
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Add relationship-aware dimension table resolution

In `src/engine/model.rs`, add a new method `dim_table_for_discovery()` that
returns the correct physical table for metadata queries (distinct value
enumeration, cardinality). When a dimension has an explicit `table_name`,
return it. Otherwise, look up a relationship via `rel_for_dimension()` and
return the relationship's `dim_table`. Fall back to `primary_table_name()`
only when neither exists.

```rust
/// The physical table for member/distinct-value discovery.
/// Uses the relationship-backed dimension table when configured,
/// falling back to the primary fact table only when no relationship exists.
pub fn dim_table_for_discovery(&self, dim_id: &str) -> &str {
    let dim = self.dim_def(dim_id);
    if let Some(ref table_name) = dim.table_name {
        return table_name;
    }
    if let Some(rel) = self.rel_for_dimension(dim_id) {
        return &rel.dim_table;
    }
    self.primary_table_name()
}
```

**Verify**: `cargo build --lib` → exit 0, no errors.

### Step 2: Use relationship-aware resolution in member discovery

In `src/xmla/discover/members.rs`, replace `model.dim_table(&dim.id)` with
`model.dim_table_for_discovery(&dim.id)` at lines 55 and 85.

**Verify**: `cargo build --lib` → exit 0.

### Step 3: Gate unsupported SQL fallback measures

In `src/engine/plan.rs`, add a helper to classify fallback SQL capability
and reject unsupported usage.

Add to `SemanticModel` (in `src/engine/model.rs`) an enum and method:
```rust
/// The shape a fallback SQL query is compatible with.
pub enum FallbackShape {
    /// Scalar only — works for Total plans, not GroupBy.
    ScalarOnly,
    /// Full — the SQL text contains filters/grouping or is expected
    /// to work for all plan shapes.
    Full,
    /// Placeholder — the SQL is a TODO stub; do not execute.
    Stub,
}

pub fn classify_fallback(&self, meas_id: &str) -> Option<FallbackShape> {
    let meas = self.meas_def(meas_id);
    let sql = meas.sql_fallback_sql.as_deref()?;
    let upper = sql.to_uppercase();
    if upper.trim() == "SELECT 1 AS DUMMY;" || upper.contains("TODO") {
        return Some(FallbackShape::Stub);
    }
    // Heuristic: if the SQL is a single SELECT without GROUP BY,
    // it's likely scalar-only.
    if !upper.contains("GROUP BY") {
        return Some(FallbackShape::ScalarOnly);
    }
    Some(FallbackShape::Full)
}
```

In `execute_plan_with_backend()` (`src/engine/plan.rs:235-270`), gate the
fallback path. Rejected fallback shapes must NOT silently fall through to
generated SQL — that can produce wrong answers. Return `QueryResult::Empty`
and log the reason instead:

```rust
let fallback_result = match plan {
    QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => {
        match model.classify_fallback(measure) {
            Some(FallbackShape::Stub) => {
                eprintln!("plan: measure '{}' fallback SQL is a TODO stub — returning empty", measure);
                Some(QueryResult::Empty)
            }
            Some(FallbackShape::ScalarOnly) => match plan {
                QueryPlan::Total { .. } => None, // scalar is fine for totals
                QueryPlan::GroupBy { .. } => {
                    eprintln!("plan: measure '{}' fallback SQL is scalar-only, cannot satisfy GroupBy — returning empty", measure);
                    Some(QueryResult::Empty)
                }
                _ => Some(QueryResult::Empty),
            },
            Some(FallbackShape::Full) => None, // let the fallback SQL run
            None => None, // no fallback SQL at all — generate normally
        }
    }
    _ => None,
};

if let Some(early) = fallback_result {
    return early;
}

let sql = match plan {
    QueryPlan::Total { measure, .. } | QueryPlan::GroupBy { measure, .. } => {
        model.meas_def(measure).sql_fallback_sql.as_deref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| sql_for_query_plan(model, plan))
    }
    _ => sql_for_query_plan(model, plan),
};
```

**Verify**: `cargo build --lib` → exit 0.  
**Verify**: `cargo test --lib excel_trace_` → 19 pass (no existing fallback measures are used by the replay suite, so this should be a no-op for current tests).

### Step 4: Add generated-project load and structure tests

In `src/project/project.rs` `mod tests`, add tests that load the generated
project and verify basic structure:

```rust
#[test]
fn generated_project_loads() {
    let p = ProxyProject::load("generated_project/proxy-config.json")
        .expect("load generated_project");
    assert_eq!(p.config.catalog, "SEMANTICMODEL");
    assert_eq!(p.config.cube, "DW_FYS_F_UNDERSÖKNING");
    assert!(!p.model.fact_tables.is_empty());
    assert!(p.model.dimensions.len() >= 10, "should have many dimensions");
    assert!(p.model.measures.len() >= 20, "should have many measures");
    assert!(!p.model.relationships.is_empty(), "should have relationships");
}

#[test]
fn generated_project_picks_one_non_fallback_measure() {
    let p = ProxyProject::load("generated_project/proxy-config.json")
        .expect("load generated_project");
    // Find a simple Malloy measure (no sql_fallback_file).
    let simple = p.model.measures.iter()
        .find(|m| m.sql_fallback_sql.is_none())
        .expect("at least one non-fallback measure exists");
    assert!(!simple.semantic_name.is_empty());
    assert!(!simple.physical_expr.is_empty());
}

#[test]
fn generated_project_relationship_backed_dimension_has_correct_table() {
    let p = ProxyProject::load("generated_project/proxy-config.json")
        .expect("load generated_project");
    // Pick a dimension that has a relationship but no fact_table binding.
    let rel_dim = p.model.dimensions.iter().find(|d| {
        d.table_name.is_none() && p.model.rel_for_dimension(&d.id).is_some()
    }).expect("at least one relationship-backed dimension exists");
    let resolved = p.model.dim_table_for_discovery(&rel_dim.id);
    let rel = p.model.rel_for_dimension(&rel_dim.id).unwrap();
    assert_eq!(resolved, rel.dim_table,
        "dim_table_for_discovery should return the relationship's dim_table");
}
```

**Verify**: `cargo test --lib generated_project_loads` → pass  
**Verify**: `cargo test --lib generated_project_picks_one_non_fallback_measure` → pass  
**Verify**: `cargo test --lib generated_project_relationship_backed_dimension_has_correct_table` → pass  

### Step 5: Run full verification

**Verify**: `cargo test --lib` → all pass (208+ tests)  
**Verify**: `cargo test --lib excel_trace_` → 19 pass  
**Verify**: `cargo test --lib project::` → all project tests pass (+3 new)

## Test plan

- New tests in `src/project/project.rs` `mod tests`:
  - `generated_project_loads` — basic load + structure assertions
  - `generated_project_picks_one_non_fallback_measure` — proves non-fallback measures exist
  - `generated_project_relationship_backed_dimension_has_correct_table` — proves
    relationship-aware table resolution works for a real dimension
- Pattern after existing project tests:
  - `third_project_loads` at `src/project/project.rs:358`
  - Uses `ProxyProject::load()` pattern
- Verification: `cargo test --lib` → all pass, including N+3 new tests.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib generated_project_loads` passes
- [ ] `cargo test --lib generated_project_picks_one_non_fallback_measure` passes
- [ ] `cargo test --lib generated_project_relationship_backed_dimension_has_correct_table` passes
- [ ] `cargo test --lib` exits 0 (all 208+ tests pass)
- [ ] `cargo test --lib excel_trace_` exits 0 (19 tests pass, no regression)
- [ ] `grep -rn "dim_table(" src/xmla/discover/members.rs` shows
  `dim_table_for_discovery(` instead (no raw `dim_table` call)
- [ ] No files outside the in-scope list are modified
- [ ] `plans/README.md` status row for plan 004 updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the excerpts
  (the codebase has drifted since this plan was written).
- `generated_project/proxy-config.json` is not found (the artifact may have
  been regenerated or moved).
- The generated project has no non-fallback measures, or no relationship-
  backed dimensions — the conversion artifacts may have changed shape.
- Any existing Excel replay test (`excel_trace_*`) breaks after the fallback
  gating change — the heuristic classification may be too aggressive.
- `dim_table_for_discovery` returns a different table name than expected
  for the relationship-backed dimension.
- A step's verification fails twice after a reasonable fix attempt.

## Maintenance notes

- `dim_table_for_discovery()` is the metadata/debug path. The SQL emitter in
  `src/engine/sql.rs` already uses `rel_for_dimension()` for JOIN generation
  — that path is correct and does not need this change.
- The fallback classification heuristic (`GROUP BY` presence) is deliberately
  conservative and fails closed: scalar-only fallbacks refuse grouped plans
  rather than silently substituting generated SQL. If a real fallback file
  is scalar-only but should support grouping (e.g. by using window functions),
  it must be promoted to `Full` with an explicit marker in
  `proxy-config.json` rather than relying on auto-classification.
- This plan does NOT populate `DimensionDef.table_name` from relationships
  at model-build time because that would change the contract for SQL
  emission (which treats `None` table_name + relationship as a JOIN case).
  The separate `dim_table_for_discovery()` method keeps metadata and query
  emitters cleanly separated.
- Full Excel smoke testing against a real DuckDB instance for
  `generated_project` requires data loading — that is out of scope here
  but should follow this plan as a manual verification step.
