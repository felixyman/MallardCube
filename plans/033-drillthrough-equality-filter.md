# Plan 033: DRILLTHROUGH filters are exact (level-aware)

## Status

- **Priority**: P1 (correctness — "Show Details" returned wrong rows)
- **Effort**: M (the plan's original "replace LIKE with equality" was not enough)
- **Risk**: LOW
- **Depends on**: none (042 made the path testable with real backends)
- **Category**: correctness
- **Status**: **DONE 2026-09-20**

## Why this mattered

DRILLTHROUGH built its WHERE clause with `CAST(col AS VARCHAR) LIKE 'key%'`,
which is not an equality filter:

- **Flat dimensions over-matched.** Live proof on the demo data:
  `DRILLTHROUGH ... WHERE ([Territory].[Territory].&[North])` returned
  `{North: 333, Northeast: 334, Northwest: 333}` — "Show Details" on a North
  cell showed other territories' rows. `North` matches 7,500 fact rows instead
  of 2,500.
- **Compound members never matched.** `[Date].[Date].[Quarter].&[2024]&[1]`
  produced `date_key LIKE '2024|1%'` — zero rows.
- **Leaf members mismatched.** A leaf date key (`2024-01-15`) was compared
  against `date_key` (`20240115`), so leaf-level drillthrough returned nothing.
- The prefix behaviour was load-bearing for *coarse* date members only
  (`Year 2024` → `date_key LIKE '2024%'`), which is why a plain equality swap
  would have broken years.

## Design (implemented)

`src/execute/dispatch.rs` now parses each WHERE member into
`DrillMember { dim, level, keys }` (`parse_member_ref`) and builds an exact
predicate (`member_filter_sql`):

- **Multi-level dimensions** scope through their dim table by level columns,
  aligning the key parts backwards from the target level:
  ```sql
  date_key IN (SELECT date_key FROM date_dim WHERE year = '2024')
  date_key IN (SELECT date_key FROM date_dim WHERE year = '2024' AND quarter = '1')
  date_key IN (SELECT date_key FROM date_dim WHERE full_date = '2024-01-15')
  ```
  `CAST(<column> AS VARCHAR) = '<key>'` keeps the comparison type-agnostic
  (INT, DATE, VARCHAR columns all work). A bare member (`[Dim].[Hier].&[k]`)
  targets the leaf level.
- **Flat dimensions** compare the relationship's fact column (or the
  dimension's `physical_field`) by exact equality.
- **Fail closed**: a member shape that cannot be aligned falls back to exact
  equality on the fact column — never a prefix match.
- Compound keys are consumed as one member, so the scan continues past
  `&[2024]&[1]` correctly.

## Verification

- Live (demo data, rebuilt server):
  | Query | Result |
  |---|---|
  | `[Territory].&[North]` | 1,000 rows, **all North** (was 333/334/333) |
  | `[Date].[Date].[Year].&[2024]` | 1,000 rows, all 2024 |
  | `[Date].[Date].[Quarter].&[2024]&[1]` | 734 rows, all 2024 Q1 (was 0) |
  | bare `DRILLTHROUGH` | 1,000 rows (unchanged) |
- Tests: `drillthrough_flat_dimension_member_is_exact`,
  `drillthrough_coarse_date_member_scopes_exactly`,
  `drillthrough_compound_quarter_member_scopes_exactly`, plus the existing
  file-backed Contoso test (1,000-row LIMIT) — 423 tests green.
- Removed the now-dead `first_bracket` helper (and its tests).

## Notes / follow-ups

- `CAST(col AS VARCHAR) = ...` is not sargable on the fact column; for
  relationship-backed dimensions the subquery runs against the (small) dim
  table, and for flat dimensions a full scan was already the case. A numeric
  fast path (`col = 123`) can be added later if a large fact table makes it
  worthwhile.
