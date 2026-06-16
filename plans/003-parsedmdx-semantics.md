# Plan 003: Move MDX semantics from string scans onto ParsedMdx structure

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a93b239..HEAD -- src/mdx/parser.rs src/mdx/semantic.rs src/execute/dispatch.rs src/engine/plan.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: 001, 002 (both should land first so their safer parser
  fixes are in place before the wider refactor)
- **Category**: tech-debt
- **Planned at**: commit `a93b239`, 2026-06-15

## Why this matters

Four functions in `src/mdx/semantic.rs` currently re-interpret the raw MDX
string using their own string scans instead of consuming the structured
`ParsedMdx` already produced by the nom parser in `src/mdx/parser.rs`.
This is the root cause that makes filtering, dimension detection, and
collapse classification brittle: a change in one Excel MDX shape can
silently affect another because each semantic helper has its own incomplete
parser.

Two of these functions also carry fallback tokens tied to the old demo
project (`"Produktkategori"`, `"FROM [Model]"`), so they break silently
when loaded against a different cube. Moving the work into `ParsedMdx`
makes the parser the single source of truth and eliminates the fallback
paths that hide errors today.

## Current state

- `src/mdx/parser.rs:392-414` — `ParsedMdx` already has fields that semantic
  helpers currently re-derive from raw strings:
  ```rust
  pub struct ParsedMdx {
      pub dim_props: Vec<String>,
      pub cell_props: Vec<String>,
      pub has_rows: bool,
      pub has_cols: bool,
      pub has_crossjoin: bool,
      pub has_drilldown: bool,
      pub has_dot_members: bool,
      pub has_dot_children: bool,
      pub has_with_member_cchildren: bool,
      pub has_where_all_measure: bool,
      pub has_drilldown_member: bool,
      pub has_measures: bool,
      pub where_members: Vec<MemberRef>,
      pub subquery_members: Vec<MemberRef>,
      pub main_dim: DimRef,
      pub cchildren_target: CChildrenTarget,
      pub calculated_members_pat: CalculatedMembersPat,
      pub selected_measure: Option<String>,
  }
  ```
  Missing fields needed for semantic consumption: axis dimension order list,
  excluded member set, collapse hierarchy target, `before_from` cube boundary.

- `src/mdx/semantic.rs:208-220` — `row_dimension_from_mdx()` scans the raw MDX
  string for configured dimension IDs, hardcodes `FROM [Model]` fallback:
  ```rust
  fn row_dimension_from_mdx(mdx: &str) -> Option<String> {
      let project = crate::proxy_project::project();
      let select_end = mdx.find(&format!("FROM [{}]", project.config.cube))
          .or_else(|| mdx.find("FROM [Model]"))
          .unwrap_or(mdx.len());
      let select_part = &mdx[..select_end];
      for dim in &project.model.dimensions {
          if select_part.contains(&format!("[{}]", dim.id)) {
              return Some(dim.id.clone());
          }
      }
      None
  }
  ```

- `src/mdx/semantic.rs:222-243` — `parse_axis_dimensions()` re-scans the
  select part for configured dimension IDs.

- `src/mdx/semantic.rs:303-332` — `parse_excluded_members()` scans the raw
  MDX string for `{-{` and every subsequent `&[...]` (addressed in plan 002).

- `src/mdx/semantic.rs:334-347` — `parse_drilldown_member_hierarchy()` scans
  the raw MDX string for `{-{` and a following `[...]` token.

- `src/mdx/parser.rs:418-422` — `before_from` detection is hardcoded to
  four specific cube names:
  ```rust
  let before_from = input.find("FROM [Model]")
      .or_else(|| input.find("FROM [model]"))
      .or_else(|| input.find("FROM [Sales]"))
      .or_else(|| input.find("FROM [sales]"))
      .map(|i| &input[..i]).unwrap_or(input);
  ```

- `src/mdx/semantic.rs:290-300` — `semantic_query_from_mdx()` delegates 5 of 11
  fields to raw-string helpers instead of using `parsed`:
  ```rust
  SemanticQuery {
      // ...
      row_dimension: row_dimension_from_mdx(mdx),           // string scan
      axis_dimensions: parse_axis_dimensions(mdx),          // string scan
      excluded_members: parse_excluded_members(mdx),        // string scan
      drilldown_member_hierarchy: parse_drilldown_member_hierarchy(mdx), // string scan
      measure: parsed.selected_measure.clone(),             // from ParsedMdx ✓
  }
  ```

Repo conventions and commands: same as plans 001/002.

## Commands you will need

| Purpose | Command                          | Expected on success            |
|---------|----------------------------------|--------------------------------|
| Build   | `cargo build --lib`              | exit 0, no errors              |
| Tests   | `cargo test --lib`               | all pass (198+ at time of plan)|
| Focused | `cargo test --lib excel_trace_`  | 19 pass                        |
| Focused | `cargo test --lib mdx::parser`   | all parser tests pass          |
| Focused | `cargo test --lib semantic_`     | all semantic tests pass        |
| Focused | `cargo test --lib collapse_`     | all collapse tests pass        |

## Scope

**In scope** (the only files you should modify):
- `src/mdx/parser.rs` — add fields to `ParsedMdx`, make `before_from`
  generic, parse axis dimension order and excluded members
- `src/mdx/semantic.rs` — delete the four raw-string helpers, consume
  new `ParsedMdx` fields instead
- `src/execute/dispatch.rs` tests — add characterization tests for the
  moved semantics (axis order, excluded members via `semantic_query_from_mdx`)
- `src/test_support/fixtures.rs` — may add one MDX constant for a
  non-standard cube name if needed

**Out of scope** (do NOT touch):
- `src/engine/plan.rs` — planning consumes `SemanticQuery` fields; those
  fields keep the same types, only their *source* changes
- `src/execute/render.rs`, `src/execute/axis_members.rs` — render paths
  are unchanged
- `src/xmla/` — metadata rowsets are not affected by this refactoring
- Any changes to `SemanticQuery` field types — this plan moves the derivation
  of existing field values, it does not change what those fields contain

## Git workflow

- Branch: `advisor/003-parsedmdx-semantics`
- Commit per step.
- Commit message style: "refactor: ..."
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Make the parser cube-agnostic

In `src/mdx/parser.rs:416-422`, replace the hardcoded cube-name search with
a generic `FROM [...` parse. Strategy: find `FROM [` (case-insensitive),
then extract the bracket contents as the cube name.

Implementation sketch:
```rust
pub fn parse_mdx(input: &str) -> ParsedMdx {
    let up = input.to_uppercase();

    // Find `FROM [` boundary generically (case-insensitive).
    let before_from = up.find("FROM [")
        .map(|i| &input[..i])
        .unwrap_or(input);

    // Extract cube name from `FROM [cubeName]`.
    let cube_name: Option<String> = up.find("FROM [").and_then(|start| {
        let after_from = &input[start + "FROM [".len()..];
        after_from.find(']').map(|end| after_from[..end].to_string())
    });
    // ...
}
```

Add a `cube_name: Option<String>` field to `ParsedMdx`. Populate it in
`parse_mdx()`.

**Verify**: `cargo build --lib` → exit 0.  
**Verify**: `cargo test --lib mdx::parser` → all pass.

### Step 2: Parse axis dimension order in the parser

In `src/mdx/parser.rs`, parse the select clause (text before `FROM [cube]`)
and find all dimension references in positional order.

The current `parse_axis_dimensions()` in semantic.rs works by scanning the
select clause for configured dimension IDs and preserving positional order
by occurrence. The parser replacement must match this exactly: extract
dimension identifiers from the select clause in positional order, but the
semantic layer will still filter them against the project model (because
the parser has no project context). This means the parser should capture
**all** `[<id>]` tokens that appear as dimension references, not just the
ones recognized by the current project.

Strategy (more precise than "scan all bracket IDs"):
- Split `before_from` on `CrossJoin(` and `DrilldownLevel(` to find axis
  expression boundaries.
- Within each axis expression, find the first bracketed identifier after
  `{` that is not `[Measures]`. That identifier is the dimension reference.
- Collect in left-to-right order (the order they appear in the select clause).

Add a field `axis_dimension_ids: Vec<String>` to `ParsedMdx` (raw identifiers
from the select clause, unfiltered).

**Verify**: `cargo build --lib` → exit 0.  
**Verify**: Add a parser test that checks raw axis IDs from a CrossJoin MDX and a DrilldownLevel MDX.

### Step 2b: Parity lock — parser output must match current parse_axis_dimensions()

Before deleting the semantic helpers, add a characterization test that
asserts the new parser field produces the same output as the existing
`parse_axis_dimensions()` function for every Excel replay fixture.

In `src/execute/dispatch.rs` `mod tests`, add:

```rust
#[test]
fn parser_axis_dimension_ids_match_semantic_parse_axis_dimensions() {
    with_project3(|| {
        for mdx in EXCEL_TRACE_PROJECT3_EXECUTES {
            // Skip member/children probes — they don't have axis dimensions.
            if mdx.contains(".Members") || mdx.contains(".Children") || mdx.contains("AddCalculatedMembers") {
                continue;
            }
            let parsed = crate::mdx_parser::parse_mdx(mdx);
            let from_parser: Vec<String> = parsed.axis_dimension_ids.iter()
                .filter(|id| crate::proxy_project::project().model.dim_def_opt(id).is_some())
                .cloned()
                .collect();
            let from_semantic = crate::mdx_semantic::parse_axis_dimensions(mdx);
            assert_eq!(from_parser, from_semantic,
                "axis dimension mismatch for: {mdx}");
        }
    });
}
```

This test must pass BEFORE step 4 proceeds. If any fixture produces a
mismatch, the parser extraction logic in step 2 needs revision.

**Verify**: `cargo test --lib parser_axis_dimension_ids_match_semantic_parse_axis_dimensions` → pass

### Step 3: Parse excluded members in the parser

In `src/mdx/parser.rs`, move the bounded excluded-member scan (as fixed in
plan 002) into the parser. When `has_drilldown_member` is true, parse the
exclusion set `{-{...}}` and extract the member keys and dimensions.

Add a field `excluded_members: Vec<(String, String)>` to `ParsedMdx` where
each tuple is `(dimension_id, member_key)`.  Keep dimension resolution in
the parser local — just capture the dimension token from the member
reference (e.g. `[Territory]` from `[Territory].[Territory].&[Northwest]`).

Also add `drilldown_member_hierarchy: Option<String>` for the dimension
token following the exclusion set's `}}`.

**Verify**: `cargo build --lib` → exit 0.  
**Verify**: Add a parser test for excluded-member extraction from a DrilldownMember MDX.

### Step 4: Remove the four raw-string helpers from semantic.rs

In `src/mdx/semantic.rs`, delete the four functions:
- `row_dimension_from_mdx()`
- `parse_axis_dimensions()`
- `parse_excluded_members()`
- `parse_drilldown_member_hierarchy()`

In `semantic_query_from_mdx()`, replace their call sites with fields from
`parsed`:

```rust
SemanticQuery {
    // ...
    row_dimension: parsed.axis_dimension_ids.iter()
        .find(|id| project.model.dim_def_opt(id).is_some())
        .cloned(),
    axis_dimensions: parsed.axis_dimension_ids.iter()
        .filter(|id| project.model.dim_def_opt(id).is_some())
        .cloned()
        .collect(),
    excluded_members: parsed.excluded_members.iter().map(|(dim, key)| {
        ExcludedMember { dimension: dim.clone(), key: key.clone() }
    }).collect(),
    drilldown_member_hierarchy: parsed.drilldown_member_hierarchy.clone(),
    measure: parsed.selected_measure.clone(),  // unchanged
}
```

**Verify**: `cargo build --lib` → exit 0. No warnings from removed dead code
(the four deleted functions should have no other callers — confirm with
`grep -rn` before deleting).

### Step 5: Verify against the full replay suite

**Verify**: `cargo test --lib excel_trace_` → 19 pass (all real Excel MDX
still returns identical SemanticQueries and cellsets).

**Verify**: `cargo test --lib` → all pass (200+ tests, no regressions).

If any Excel replay test breaks, compare the before/after `SemanticQuery`
for the failing MDX — the fields should be identical. If they differ, the
parser extraction logic in step 2 or 3 may have a positional or dimension-
resolution mismatch.

## Test plan

- New parser tests in `src/mdx/parser.rs` `mod tests`:
  - Cube name extraction: `FROM [Sales]` → `Some("Sales")`, `FROM [DW_FYS_F_UNDERSÖKNING]` → correct
  - Axis dimension order from a CrossJoin MDX: `[Category, Territory]`
  - Axis dimension order from a DrilldownLevel MDX: `[Territory]`
  - Excluded member extraction from DrilldownMember MDX: `[("Territory", "Northwest")]`
  - Excluded members from a collapse MDX with trailing WHERE slicers: still only the exclusion set members
- Pattern after: existing parser tests at `src/mdx/parser.rs:471-588`
- Verification: `cargo test --lib mdx::parser` → all pass with N+4 new tests.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib parser_axis_dimension_ids_match_semantic_parse_axis_dimensions` passes (parity gate cleared before helper deletion)
- [ ] `cargo test --lib excel_trace_` exits 0 (19 tests pass)
- [ ] `cargo test --lib` exits 0 (all 200+ tests pass)
- [ ] `grep -rn "FROM \[Model\]" src/mdx/parser.rs` returns no matches
- [ ] `grep -rn "parse_excluded_members\|parse_axis_dimensions\|row_dimension_from_mdx\|parse_drilldown_member_hierarchy" src/mdx/semantic.rs` returns no matches
- [ ] No files outside the in-scope list are modified
- [ ] `plans/README.md` status row for plan 003 updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at the locations in "Current state" doesn't match the excerpts
  (the codebase has drifted since this plan was written).
- Any Excel replay test (`excel_trace_*`) breaks and the root cause is not
  a trivial positional or casing mismatch that can be fixed within the
  parser extraction logic.
- The `parser_axis_dimension_ids_match_semantic_parse_axis_dimensions` parity
  test does not pass — stop and report which MDX fixtures produce mismatches.
  Do NOT proceed to step 4 (delete helpers) until parity is achieved.
- A step's verification fails twice after a reasonable fix attempt.
- The `cube_name` extraction fails for any existing replay MDX — stop and
  report the MDX string and the parser state.

## Maintenance notes

- After this plan, `ParsedMdx` is the single source of truth for MDX
  structure. Any future semantics (time intelligence, calculated members,
  named sets) should add fields to `ParsedMdx` and consume them in
  `semantic_query_from_mdx()` — do not add new raw-string helpers.
- The `axis_dimension_order` field captures positionally-ordered dimension
  IDs from the select clause. The semantic layer still resolves these
  against the project's configured dimensions (filtering out IDs not in
  the project model). If a project has dimension IDs that don't appear in
  MDX member references, that resolution stays in semantic.rs.
- `cube_name` is exposed but not yet consumed by other code paths — it
  replaces the hardcoded cube search in `parse_mdx()` and enables
  cube-aware error messages later without another parser change.
