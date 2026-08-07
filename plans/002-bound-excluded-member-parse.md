# Plan 002: Bound excluded-member parsing to the DrilldownMember exclusion set

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a93b239..HEAD -- src/mdx/semantic.rs src/execute/dispatch.rs src/test_support/fixtures.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: MED
- **Depends on**: none (independent of plan 001, but both touch MDX parsing)
- **Category**: bug
- **Planned at**: commit `a93b239`, 2026-06-15

## Why this matters

When Excel sends a `DrilldownMember` collapse query, the current parser
finds the exclusion set marker `{-{` and then scans *every subsequent* `&[`
token in the rest of the MDX. In real Excel traffic, collapse queries can
also carry leaf-member slicers in the same `WHERE` clause (e.g.
`[Segment].&[Consumer]`). Today those later slicer members are mistakenly
absorbed as collapse exclusions, polluting `SemanticQuery.excluded_members`.
This hasn't broken the demo yet because the over-parsed dimensions don't
overlap with the current axis dimensions — but it *will* break as soon as a
collapsed dimension also appears as a slicer dimension, and it already
produces incorrect semantic state.

## Current state

- `src/mdx/semantic.rs:303-332` — `parse_excluded_members()`:
  ```rust
  fn parse_excluded_members(mdx: &str) -> Vec<ExcludedMember> {
      let model = &crate::proxy_project::project().model;
      let default_dim = model.default_dimension_id()
          .unwrap_or_else(|| "Produktkategori".into());
      let mut result = Vec::new();
      let Some(excl_start) = mdx.find("{-{") else { return result; };
      let excl = &mdx[excl_start..];
      let mut search_from = 0;
      while let Some(amp) = excl[search_from..].find("&[") {
          // ...scans ALL subsequent &[...] tokens in the rest of the MDX...
      }
      result
  }
  ```
  The loop runs on `excl[start..]` which is the entire MDX tail — it never
  stops at the end of the `DrilldownMember` expression. Later slicers with
  `&[...]` keys get picked up as exclusions.

- `src/test_support/fixtures.rs:80` — the real Excel collapse fixture
  `EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE` includes one
  excluded member plus later `WHERE` slicers:
  ```sql
  ...DrilldownMember({{[Territory].[Territory].[All]}},{-{[Territory].[Territory].&[Northwest]}})...
  ...WHERE ([Segment].[Segment].&[Consumer],[Channel].[Channel].&[Wholesale],[Measures].[Revenue])...
  ```
  The current parser would also pick up `Consumer` and `Wholesale` as
  exclusions from the `WHERE` clause, associating them with whatever
  default dimension the backward scan resolves to.

- `src/mdx/semantic.rs:334-347` — `parse_drilldown_member_hierarchy()` already
  correctly finds the closing `}}` of the exclusion set. The fix for
  `parse_excluded_members` should follow the same pattern.
  ```rust
  fn parse_drilldown_member_hierarchy(mdx: &str) -> Option<String> {
      let Some(excl_start) = mdx.find("{-{") else { return None; };
      let after_excl = &mdx[excl_start..];
      let Some(close) = after_excl[2..].find("}}") else { return None; };
      let rest = &after_excl[2 + close + 2..];
      // ...
  }
  ```

- `src/execute/dispatch.rs` — existing collapse tests at ~line 500–640 verify
  the rendered cellset shape but not the exact `excluded_members` list.

Repo conventions: same as plan 001 — `cargo test --lib`, test helpers in
`mod tests`, `with_project3()` for project3-specific tests.

## Commands you will need

| Purpose | Command                          | Expected on success            |
|---------|----------------------------------|--------------------------------|
| Build   | `cargo build --lib`              | exit 0, no errors              |
| Tests   | `cargo test --lib`               | all pass (197 at time of plan) |
| Focused | `cargo test --lib excel_trace_`  | 19 pass                        |
| Focused | `cargo test --lib collapse_`     | all collapse-related pass      |

All commands run from repo root `/home/felix/code/MallardCube`. Source
`$HOME/.cargo/env` before running if `cargo` is not on `$PATH`.

## Scope

**In scope** (the only files you should modify):
- `src/mdx/semantic.rs` — bound the `&[...]` scan in `parse_excluded_members()`
- `src/execute/dispatch.rs` tests — add one characterization test

**Out of scope** (do NOT touch):
- `src/mdx/parser.rs` — the `has_drilldown_member` flag is already set
  correctly; the parser is not involved in exclusion parsing
- `src/execute/render.rs` — collapse rendering logic is unchanged; this
  plan only fixes what members are considered excluded
- Any *new* collapsed-member MDX queries; this plan bounds the parser to
  what the real Excel fixture already contains

## Git workflow

- Branch: `advisor/002-bound-excluded-member-parse`
- Commit per step.
- Commit message style: "fix: bound excluded-member parsing ..."
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Bound the `&[` scan to the exclusion set

In `src/mdx/semantic.rs:303-332`, modify `parse_excluded_members()` to stop
scanning at the closing `}}` of the `DrilldownMember` exclusion set, matching
the pattern already used by `parse_drilldown_member_hierarchy()`.

Target logic:
1. Find `{-{` as before.
2. Find the matching `}}` after the exclusion members.
3. Only scan `&[...]` tokens within that bounded slice.
4. Keep the backward dimension-resolution logic unchanged.

Implementation sketch:
```rust
fn parse_excluded_members(mdx: &str) -> Vec<ExcludedMember> {
    let model = &crate::proxy_project::project().model;
    let default_dim = model.default_dimension_id()
        .unwrap_or_else(|| "Produktkategori".into());
    let mut result = Vec::new();

    let Some(excl_start) = mdx.find("{-{") else { return result; };

    // Locate the closing }} of the exclusion set so we don't scan
    // past it into later WHERE slicers.
    let after_excl = &mdx[excl_start..];
    let Some(close) = after_excl[2..].find("}}") else { return result; };
    let excl_end = 2 + close + 2;
    let excl = &after_excl[..excl_end];

    let mut search_from = 0;
    while let Some(amp) = excl[search_from..].find("&[") {
        let begin = search_from + amp + 2;
        if let Some(end) = excl[begin..].find(']') {
            let key = excl[begin..begin + end].to_string();
            // ... same backward dimension resolution as before ...
            let before = &excl[..search_from + amp];
            let dim = if let Some(last_dot) = before.rfind("].") {
                if let Some(open) = before[..last_dot].rfind('[') {
                    let raw = &before[open + 1..last_dot];
                    model.lookup_dimension(raw)
                        .map(|d| d.id.clone())
                        .unwrap_or_else(|| raw.to_string())
                } else { default_dim.clone() }
            } else { default_dim.clone() };
            result.push(ExcludedMember { dimension: dim, key });
            search_from = begin + end;
        } else {
            break;
        }
    }
    result
}
```

**Verify**: `cargo build --lib` → exit 0, no errors.

### Step 2: Add a characterization test for excluded-member scoping

In `src/execute/dispatch.rs` `mod tests`, add a test that asserts the exact
excluded-member count and content for the real Excel collapse fixture:

```rust
#[test]
fn collapse_parse_only_excludes_the_drilldownmember_members() {
    with_project3(|| {
        use crate::test_fixtures::EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE;
        let query = crate::mdx_semantic::semantic_query_from_mdx(
            EXCEL_TRACE_TERRITORY_CATEGORY_COLLAPSE_NORTHWEST_REVENUE
        );
        // Only the one explicit exclusion from DrilldownMember, not the
        // later slicer members for Segment/Channel.
        assert_eq!(query.excluded_members.len(), 1,
            "should only exclude the DrilldownMember member, not slicer members");
        assert_eq!(query.excluded_members[0].key, "Northwest");
        assert_eq!(query.excluded_members[0].dimension, "Territory");
    });
}
```

**Verify**:
```
cargo test --lib collapse_parse_only_excludes_the_drilldownmember_members
```
→ pass

### Step 3: Run full verification

**Verify**: `cargo test --lib` → all pass (198+ tests)
**Verify**: `cargo test --lib excel_trace_` → 19 pass
**Verify**: `cargo test --lib collapse_` → all pass

## Test plan

- New characterization test in `src/execute/dispatch.rs` `mod tests`:
  - `collapse_parse_only_excludes_the_drilldownmember_members` — asserts
    exactly 1 exclusion for the real Excel collapse MDX, with correct key
    and dimension, proving slicer members are not absorbed.
- Pattern after existing tests:
  - `parse_excluded_members_detects_region_dimension` in dispatch.rs (~line 612)
- Verification: `cargo test --lib` → all pass, including N+1 new test.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib collapse_parse_only_excludes_the_drilldownmember_members` passes
- [ ] `cargo test --lib excel_trace_` exits 0 (19 tests pass)
- [ ] `cargo test --lib` exits 0 (all 198+ tests pass)
- [ ] No files outside the in-scope list are modified
- [ ] `plans/README.md` status row for plan 002 updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at `src/mdx/semantic.rs:303-332` doesn't match the excerpt in
  "Current state" (the codebase has drifted).
- The collapse rendering test (`excel_trace_crossjoin_collapse_rolls_up_northwest_total`)
  breaks after the fix — the renderer may actually depend on the
  over-parsed excluded members for some filtering edge case.
- A step's verification fails twice after a reasonable fix attempt.
- The fix appears to require touching an out-of-scope file.

## Maintenance notes

- This fix interacts with `src/execute/render.rs:126-199` (`build_drilldown_member`)
  which uses `query.excluded_members` to determine which groups to collapse.
  If a future change adds slicer dimensions that overlap with axis dimensions,
  make sure the exclusion list remains scoped correctly.
- `parse_excluded_members()` and `parse_drilldown_member_hierarchy()` now
  both search for the closing `}}` — consider extracting a shared helper
  `find_exclusion_set_bounds(mdx) -> Option<(usize, usize)>` in a follow-up
  if either function is further modified.
