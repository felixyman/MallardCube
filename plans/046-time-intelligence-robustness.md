# Plan 046: Time intelligence robustness — the SSAS killer feature

## Status

- **Priority**: P1 (time intelligence is a headline SSAS capability; Excel users
  hit it daily)
- **Effort**: M/L across six slices; slices 1–2 are S
- **Risk**: MEDIUM (MDX set handling touches the parser/planner; slice 1 is
  metadata-only)
- **Depends on**: 045 (intake fidelity), 007 (measure-scoped date roles)
- **Category**: Excel compatibility / MDX
- **Status**: **IN PROGRESS 2026-09-21** — slices 1–3, 5 (partial) and 6 landed;
  slice 4 (named-set MDX) and the rest of 5 (`ParallelPeriod`/`LastPeriods`)
  TODO.

## Why this matters

The measure path is solid: flag-based YTD/prior-YTD/QTD/MTD, per-measure date
roles, and a live YTD query that matched a SQL oracle exactly. But the
**client/MDX path is thin**, and that is what Excel users actually touch.

## Captured evidence (2026-09-21, real MSOLAP client against the proxy)

Every row below was captured from Excel (CUBE formulas via the Excel MCP
harness; MDX from `xmla-trace.jsonl`):

| Excel action | MDX Excel emits | Result |
|---|---|---|
| `CUBESET` plain set | `HEAD([Date].[Date].[Year].Members,1)` + `WITH MEMBER [Measures].[XL_SD] AS 'COUNT(…)'` | ✅ 11 years |
| Member probes | `strtomember(…).UniqueName` / `.properties("caption")` | ✅ |
| `CUBESET` with `YTD()` | `HEAD(YTD([Date].[Date].[Year].&[2024]),1)` | ❌ empty → `#N/A` |
| Named-set sliding window | `HEAD(Filter([Date].[Date].[Date].Members, [Date].[Date].CurrentMember.Member_Value >= DateAdd('d',-30,VBA![Date]())),1)` | ❌ `#N/A` |
| `CUBESET` range | `HEAD({[Date].[Date].[Year].&[2022]:…&[2024]},1)` | ❌ `#N/A` |
| Range set on an axis | `{…&[2022]:…&[2024]} ON 1` | ❌ axis dropped |
| `PeriodsToDate(…)` on an axis | — | ❌ axis dropped |
| Named set | `WITH SET [x] AS '…' SELECT {[Measures].[Revenue]} ON 0` | ❌ silently misparsed (answers with a `[Category].[Category]` axis) |

Additional findings:

1. **Unsupported set expressions fail silently** — dropped axes or a
   wrong-hierarchy fallback, never a fault. In Excel that is an empty or bogus
   pivot with no explanation.
2. **`LEVEL_DBTYPE` is hardcoded `130` (WSTR) for every level**, including
   `[Date].[Date].[Date]` whose column is a real `DATE`. `[Date]` is correctly
   `DIMENSION_TYPE=1` and levels carry time `LEVEL_TYPE`s, but Excel refused to
   attach its own date filter via VBA (`PivotFilters.Add2 Type:=xlYearToDate` →
   1004) and refused `PivotField.DrillTo` (1004). The DBTYPE is the prime
   suspect; the fix is verifiable by retrying `Add2`.
3. VBA cannot automate the PivotTable date-filter UI on OLAP pivots, so the UI
   capture needs a human at the VM — the DBTYPE retest is the automated proxy
   for it.

## Slices

| # | Slice | Effort | Status |
|---|---|---|---|
| 1 | **Level data types**: emit the real OLE DB `LEVEL_DBTYPE` per level (date for a date role's leaf, numeric for year/quarter/month) instead of a hardcoded string | S | **DONE** |
| 2 | **Loud faults**: unsupported set expressions (ranges, time functions, `WITH SET`, member-value `Filter`, VBA functions) return a SOAP fault naming the construct instead of a dropped/bogus axis; capability negotiation stops advertising named sets | S | **DONE** |
| 3 | **Range sets** (`a : b`) in set probes (`HEAD({a:b},n)`, bare sets) | S/M | **DONE** |
| 4 | **`WITH SET` + `Filter` with member-value comparisons + `DateAdd`/VBA date functions** (the documented Excel named-set pattern) | M | TODO |
| 5 | **MDX time functions**: `YTD`/`QTD`/`MTD`/`PeriodsToDate` lowered to date windows on the anchor's date role; `ParallelPeriod`/`LastPeriods` still fault | M | **PARTIAL** |
| 6 | **Converter DAX mappings**: `TOTALQTD`/`TOTALMTD`/`DATESQTD`/`DATESMTD` → the existing qtd/mtd flags; plain aggregates lowered to real SQL (was stubs) | S | **DONE** |

## Slice 1 evidence (2026-09-21)

- `LEVEL_DBTYPE` now reflects the level: date-role leaves report
  `DBTYPE_DBTIMESTAMP` (135), year/quarter/month levels `DBTYPE_I4` (3),
  non-date dimensions stay `DBTYPE_WSTR` (130); a flat date role's leaf also
  reports a date type (`levels.rs::level_db_type`). Verified live:
  `[Date].[Date].[Date]` → 135, `Year`/`Quarter`/`Month` → 3, `[Category]` → 130.
- **Excel's own date filter still cannot be created via VBA**
  (`PivotFilters.Add2` → 1004), but neither can a *label* filter
  (`xlCaptionEquals` → 1004) — so Excel refuses pivot filters on OLAP sources
  in its object model, not date filters specifically. The UI capture therefore
  still needs a human at the VM (or a screenshot of the Date Filters menu).
- `PivotField.DataType` reports `-4145` (xlNumber) for both the Year and the
  Date level; for OLAP fields Excel does not expose a date type there, so it is
  not a useful signal.
- `PivotTable.RefreshTable` does re-query: the captured live pivot MDX is the
  supported `DrilldownLevel({[Date].[Date].[All]})` shape, returning the total
  plus years.

## Slice 2 evidence (2026-09-21)

- `unsupported_features` (`mdx/parser.rs`) detects `WITH SET`, member ranges,
  MDX time functions (`YTD`/`QTD`/`MTD`/`PeriodsToDate`/`ParallelPeriod`/
  `LastPeriods`/`ClosingPeriod`/`OpeningPeriod`), `DateAdd`/`VBA!`, and
  member-property `Filter`s; `unsupported_fault` (`execute/builders.rs`) turns
  them into a SOAP fault used by both the production and test entry points.
- Verified live: `YTD()`, ranges and `WITH SET` now return
  `<faultstring>… not supported yet</faultstring>` instead of a dropped axis or
  a wrong-hierarchy (`[Category]`) cellset. Supported queries unchanged
  (smoke 8/8, Revenue 521,586,767).
- **Capability negotiation**: Excel asks for `MdpropMdxNamedSets` at connect
  time; the proxy advertised 15 (full support) while `WITH SET` returned
  garbage. It now advertises 0 — honest until slice 4 lands.

## Slice 5 evidence (2026-09-21, partial)

- `YTD(m)`/`QTD(m)`/`MTD(m)`/`PeriodsToDate(level, m)` lower to a **date window**
  on the anchor's date role: `DateWindow { anchor: [(level, value)…], period }`
  on `DimensionFilter`/`TypedDimensionFilter`, emitted as
  `<date_col> BETWEEN date_trunc('<period>', <anchor date>) AND <anchor date>`
  with the anchor pinned by the key path aligned to the model's levels (a short
  key anchors at the anchor's level, like the range SQL).
- The anchor's level is listed on the axis / in the set probe (level drag or
  `SetExpr::LevelMembers`), so `{YTD(m)}` returns the window's members and
  `HEAD(YTD(m), n)` prunes them.
- Verified live: `HEAD(YTD([Year].&[2024]), 1)` → 2024 (the captured CUBESET
  shape), `YTD(June 2024)` → months 1–6, `QTD(June 2024)` → 4–6,
  `MTD(June 2024)` → 6, `PeriodsToDate(Year, June 2024)` → 1–6.
  `ParallelPeriod`/`LastPeriods` still fault with named reasons.
- The plan key fingerprints date windows (`anchor@period`) so the result cache
  cannot serve one window for another.
- Fiscal calendars: `date_trunc` is calendar-based — fiscal period-to-date
  belongs upstream as flag columns (plan 044 invariant 2), documented here.

## Slice 3 evidence (2026-09-21)

- `SetExpr::MemberRange { from, to }` + `parse_member_range` recognise
  `{[D].[H].[L].&[a] : [D].[H].[L].&[b]}`; the semantic layer turns it into a
  `DimensionFilter.range` and the SQL emitter compares the level column
  (`col BETWEEN 'a' AND 'b'`, literals cast to the column type, ancestors
  pinned for compound keys). The `SetMembers` plan now carries filters.
- Verified live: `HEAD({2022 : 2024}, 1)` → `2022` (Excel's CUBESET probe) and
  a bare `{2022 : 2024}` → `2022, 2023, 2024`.
- **Scope**: ranges are supported in set probes (the captured CUBESET shapes)
  and, since plan 047 increment 4, on a pivot axis beside a measure set
  (`{a : b} ON 1` returns the members between). A range in a slicer or inside a
  calculated-member `COUNT` still **faults loudly** rather than dropping the
  axis.

## Slice 6 evidence (2026-09-21)

- `classify_dax` maps `TOTALQTD`/`DATESQTD` → `time_qtd` and
  `TOTALMTD`/`DATESMTD` → `time_mtd`; measures bind to the role's
  `qtd_flag`/`mtd_flag` and are downgraded to bridge code (with the flag
  suggestion) when the calendar lacks the flag.
- **Bug fixed on the way**: `extract_ti_inner` did not strip the leading `=`,
  so converted time-intelligence measures got `sql_expr: null`.
- **Plain aggregates are now lowered** (`simple_aggregate_sql`: `SUM`, `COUNT`,
  `DISTINCTCOUNT`, `AVERAGE`, `MIN`, `MAX`, columns resolved through the
  fact table's schema mapping). This is the boundary's "plain SQL" allowance
  and it fixes the largest conversion gap: on the real export, measures with
  real SQL went **1 → 6** and stubs **23 → 18** (`COUNT(DISTINCT customer_id)`,
  `COUNT(order_id)`, `AVG(...)` now return numbers instead of blanks).

## Slice 1 design notes

- `LevelDef`/`DimensionDef` carry an OLE DB `db_type` (default `130` = WSTR).
- Source of truth, in order: (a) an explicit `db_type` on the config's
  hierarchy level (converter/AutoModel know the column types), (b) load-time
  introspection (`DESCRIBE <dim_table>` once per table, mapped to DBTYPE),
  (c) a time-level heuristic for date roles when no DB is available (leaf →
  date type, year/quarter/month → integer).
- DuckDB → DBTYPE mapping: `DATE` → 7, `TIMESTAMP*` → 135, integer types → 3
  (`BIGINT` → 20), float/decimal → 5, boolean → 11, else → 130.

## Slice 2 design notes

- A conservative pre-scan (`unsupported_mdx`) detects only constructs that are
  verified broken today, so working queries are untouched: `WITH SET`, ranges
  (`] : [`), time functions (`YTD(`/`QTD(`/`MTD(`/`PeriodsToDate(`/
  `ParallelPeriod(`/`LastPeriods(`/`ClosingPeriod(`/`OpeningPeriod(`),
  `DateAdd(`/`VBA!`, and `Filter(` whose predicate uses `Member_Value`/
  `Member_Key`/`CurrentMember`.
- The execute path returns a SOAP fault naming the construct ("MallardCube does
  not support X yet") — loud beats silently wrong.

## Done criteria

- Excel's own Date Filters can be created on a date hierarchy (retest
  `PivotFilters.Add2` after slice 1), or we know exactly why not.
- No unsupported set expression can produce a dropped axis or a wrong-hierarchy
  cellset; each faults with a named reason.
- The captured MDX shapes from the table above become regression tests as
  slices 3–5 land.
