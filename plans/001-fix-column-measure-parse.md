# Plan 001: Fix column-measure parse so ON COLUMNS measures resolve correctly

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat a93b239..HEAD -- src/mdx/parser.rs src/execute/dispatch.rs src/test_support/fixtures.rs src/engine/plan.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `a93b239`, 2026-06-15

## Why this matters

When Excel sends an MDX query that specifies a measure only in the column
axis (e.g. `{[Measures].[Units]} ON COLUMNS`) without also repeating it in
`WHERE`, the parser extracts a malformed name like `"[Units"` instead of
`"Units"`. This broken name fails the exact-match lookup in
`plan_from_semantic_with_model()` (`src/engine/plan.rs:122-123`), so the
proxy silently falls back to the default measure and renders wrong values
and wrong format strings — without any error. Fixing this prevents silent
correctness drift for any query whose measure appears only on columns.

## Current state

- `src/mdx/parser.rs` — nom-based MDX parser. Contains the bug (lines 434–446)
  and already has working `member_ref()` / `measure_member()` parsers
  (lines 69–98) that extract clean measure names. The existing test suite is
  at lines 471–588.
- `src/mdx/parser.rs:436-446` — the column-measure extraction:
  ```rust
  let selected_measure = selected_measure.or_else(|| {
      let col_start = input.find("ON COLUMNS")
          .or_else(|| input.find("on columns"))
          .unwrap_or(input.len());
      let before_cols = &input[..col_start];
      let meas_start = before_cols.find("[Measures].");
      meas_start.map(|s| {
          let rest = &before_cols[s + "[Measures].".len()..];
          rest.split(|c: char| c == ']').next().unwrap_or("").to_string()
      })
  });
  ```
  This produces `"[Revenue"` (leading `[`) from `{[Measures].[Revenue]}`.
- `src/engine/plan.rs:122-123` — measure resolution uses exact caption match:
  ```rust
  .and_then(|name| model.measures.iter().find(|m| m.caption == name).map(|m| m.id.clone()))
  ```
- `src/execute/dispatch.rs` — dispatch and Excel replay tests. Tests follow
  a pattern of helper functions in `mod tests` using `Backend::get()` for
  oracle queries, `get_execute_statement_response()` for MDX → cellset XML,
  and `with_project3()` for model override. Exemplar test at line 886.

Repo conventions:
- Rust 2021 edition, `cargo test --lib` for tests.
- No external test framework — all tests are `#[cfg(test)] mod tests` inline.
- Test helpers are private functions inside `mod tests` blocks.
- Heavily uses `format!()` and `let _lock = ...` for mutex patterns.
- Error handling: `unwrap()` in tests, `unwrap_or_else()` with reasonable
  fallbacks in production code.

## Commands you will need

| Purpose | Command                        | Expected on success            |
|---------|--------------------------------|--------------------------------|
| Build   | `cargo build --lib`            | exit 0, no errors              |
| Tests   | `cargo test --lib`             | all pass (197 at time of plan) |
| Focused | `cargo test --lib excel_trace_`| 19 pass                        |
| Focused | `cargo test --lib mdx::parser` | all parser tests pass          |

All commands run from repo root `/home/felix/code/MallardCube`. Source
`$HOME/.cargo/env` before running if `cargo` is not on `$PATH`.

## Scope

**In scope** (the only files you should modify):
- `src/mdx/parser.rs` — fix the column-measure extraction
- `src/mdx/parser.rs` tests — add a direct parser test
- `src/test_support/fixtures.rs` — add a new MDX constant if needed
- `src/execute/dispatch.rs` tests — add one regression test

**Out of scope** (do NOT touch):
- `src/mdx/semantic.rs` — this plan only fixes parser output; semantic
  consumption is unchanged
- `src/engine/plan.rs` — measure resolution logic stays as-is; it works
  correctly when the parser produces clean names
- `src/execute/render.rs`, `src/execute/axis_members.rs` — render paths
  are unaffected
- Any change to `ParsedMdx` struct fields beyond keeping
  `selected_measure` populated correctly

## Git workflow

- Branch: `advisor/001-fix-column-measure-parse`
- Commit per step.
- Commit message style: "fix: ..." (conventional commits, as seen in repo
  history: `fix:`, `feat:`, `chore:`).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Replace the substring scan with a nom parser

In `src/mdx/parser.rs`, replace lines 436–446 with a call that reuses the
existing `member_ref()` combinator to parse measure references from the
column axis expression.

The target logic:
1. Find the text before `ON COLUMNS` (keep the case-insensitive search).
2. Find the last occurrence of `[Measures].[` before that point (measures
   in a column set are at the end of the expression, before `}`).
3. Call `member_ref(...)` on a slice starting at that `[Measures]` prefix.
4. If it returns `MemberRef::Measure(name)`, use `name` as the result.

Implementation sketch (in `parse_mdx()`, replace the `or_else` block):
```rust
let selected_measure = selected_measure.or_else(|| {
    let col_start = input.find("ON COLUMNS")
        .or_else(|| input.find("on columns"))
        .unwrap_or(input.len());
    let before_cols = &input[..col_start];
    // Find the last [Measures] reference in the column expression.
    let meas_start = before_cols.rfind("[Measures].[")?;
    match member_ref(&input[meas_start..]) {
        Ok((_, MemberRef::Measure(name))) => Some(name),
        _ => None,
    }
});
```

**Verify**: `cargo build --lib` → exit 0, no errors.

### Step 2: Add a direct parser test for column-only measure extraction

In `src/mdx/parser.rs`, inside `mod tests`, add a test that parses an MDX
string where the measure appears only in the column axis:

```rust
#[test]
fn parse_selected_measure_from_columns() {
    // Measure specified on columns, not in WHERE.
    let mdx = "Select {[Measures].[Units]} on columns from [Sales]";
    let parsed = parse_mdx(mdx);
    assert_eq!(parsed.selected_measure.as_deref(), Some("Units"), "expected Units from columns");
}

#[test]
fn parse_selected_measure_from_columns_bracketed_set() {
    // Full set syntax with curly braces.
    let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]";
    let parsed = parse_mdx(mdx);
    assert_eq!(parsed.selected_measure.as_deref(), Some("Revenue"), "expected Revenue from columns");
}
```

**Verify**: `cargo test --lib mdx::parser::tests::parse_selected_measure_from_columns` → pass

### Step 3: Add an integration regression test for a column-only measure query

This verifies the full pipeline: parser → plan → execute → cellset, with a
measure that appears only in the column axis and not in WHERE.

In `src/execute/dispatch.rs` `mod tests`, add a test that uses a minimal
column-only MDX query against project3:

```rust
#[test]
fn column_only_measure_uses_correct_measure() {
    with_project3(|| {
        // Revenue specified only on columns, not in WHERE.
        let mdx = "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] CELL PROPERTIES VALUE, FORMAT_STRING, BACK_COLOR, FORE_COLOR";
        let xml = get_execute_statement_response(mdx);
        let expected = Backend::get().query_scalar("SELECT SUM(revenue) FROM sales_fact");
        assert_eq!(cell_values(&xml), vec![expected]);
        assert!(xml.contains("[Measures].[Revenue]"), "slicer axis should show Revenue");
    });
}
```

**Verify**: `cargo test --lib column_only_measure_uses_correct_measure` → pass

### Step 4: Run full verification

**Verify**: `cargo test --lib` → all 197+ tests pass (should gain 3 new tests)
**Verify**: `cargo test --lib excel_trace_` → 19 pass

## Test plan

- New direct parser tests in `src/mdx/parser.rs` `mod tests`:
  - Column-only measure extraction → `Units`
  - Column-only measure extraction with full bracket-set syntax → `Revenue`
- New integration regression in `src/execute/dispatch.rs` `mod tests`:
  - Column-only Revenue query matches SQL oracle and shows Revenue on slicer
- Pattern after existing tests:
  - Parser: `parse_measure_revenue` at `src/mdx/parser.rs:512`
  - Dispatch: `excel_trace_total_revenue_matches_raw_sql` at `src/execute/dispatch.rs:901`
- Verification: `cargo test --lib` → all pass, including N+3 new tests.

## Done criteria

- [ ] `cargo build --lib` exits 0
- [ ] `cargo test --lib mdx::parser` exits 0; new column-only measure tests pass
- [ ] `cargo test --lib column_only_measure_uses_correct_measure` passes
- [ ] `cargo test --lib` exits 0 (all 200+ tests pass)
- [ ] `cargo test --lib excel_trace_` exits 0 (19 tests pass)
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row for plan 001 updated to DONE

## STOP conditions

Stop and report back (do not improvise) if:

- The code at `src/mdx/parser.rs:436-446` doesn't match the excerpt in
  "Current state" (the codebase has drifted since this plan was written).
- The `member_ref()` call fails to parse any of the test MDX strings; the
  parser may need a broader slice than just `[Measures]` — e.g. the full
  `[Measures].[Revenue]` or `{[Measures].[Revenue]}` substring.
- Any existing Excel replay test (`excel_trace_*`) breaks; the column-only
  fix must not regress existing measure extraction from WHERE.
- A step's verification fails twice after a reasonable fix attempt.

## Maintenance notes

- The `measurement_cell` and `measures_total_member` helpers in
  `src/execute/axis_members.rs` also resolve measure IDs from the query;
  they already use the same caption/id match logic and will benefit from
  clean parser output without code changes.
- If Malloy column-measure extraction ever diverges from SQL extraction in
  `src/engine/malloy.rs`, the parser is the single source of truth for both
  paths.
- Future work on `src/mdx/parser.rs` (plan 003) will move additional
  semantics onto `ParsedMdx`; this fix is a prerequisite because it
  makes `selected_measure` reliable enough to build on.
