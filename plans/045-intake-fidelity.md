# Plan 045: Intake fidelity — converted models must arrive with their shape

## Status

- **Priority**: P1 (the launch funnel's mouth: "can I convert my model?")
- **Effort**: L (four slices, parser → config → report; slice 1 is S/M)
- **Risk**: MEDIUM (relationship semantics can change numbers; slice 1 is
  shape-only and loud)
- **Depends on**: 044 (boundary contract), 011/012 (converted-project
  qualification)
- **Category**: converter / product
- **Status**: **IN PROGRESS 2026-09-20** — slice 1 landed (hierarchy levels +
  relationship column resolution); slices 2–5 remain.

## Why this matters

The public-validation gate (G1) succeeds when real-model owners can point the
converter at their export and get something Excel-faithful. A real-model
inspection (19 tables, 33 measures, 7 date roles, 6 hierarchies) found four
fidelity gaps, ordered by how badly they hurt:

1. **Hierarchies were parsed as names only.** All three parsers kept
   `hierarchies: Vec<String>` and dropped the `levels` array, so converted
   models had no drill paths — the field list Excel users live in.
2. **Relationship endpoints were emitted as model column names** (`Customer ID`
   → `customer_id`) while `schema.sql` creates source columns (`customerid`).
   Every fact↔dim join in a fresh conversion was broken: grouped queries came
   back empty and `NON EMPTY` drills collapsed to `All`. The tracked
   `generated_retail_analytics` fixture had been hand-fixed (plans 010–012),
   which hid the bug.
3. **One global `time_intelligence.date_dimension`** cannot express
   role-playing calendars (the real model has 7).
4. **`isActive` / `crossFilteringBehavior` / cardinality are ignored** — the
   dangerous class: plausible output, wrong totals.
5. **Calculated tables/columns are dropped** without a precise report line.

## Slices

| # | Slice | Effort | Status |
|---|---|---|---|
| 1 | **Hierarchy levels + relationship columns.** Parse `levels` (BIM/folder/TMDL), emit `hierarchy_levels` and the export hierarchy name, resolve relationship endpoints through source columns, report every declared hierarchy | S/M | **DONE** |
| 2 | **Relationship semantics, loudly.** Read `isActive` / `crossFilteringBehavior` / cardinality; warn in the conversion report and fail in `qualify` wherever semantics are not implemented; then implement the safe subset (inactive relationships, many-to-one bidirectional filter propagation) | M/L | TODO |
| 3 | **Per-dimension date roles.** `time_intelligence.date_dimension` becomes a list; the converter emits one entry per date-role table | M | TODO |
| 4 | **Calculated tables/columns.** Precise report lines; materialise simple `DATATABLE` tables as static dimensions | M | TODO |
| 5 | **Gap-model fixture.** A synthetic export exercising slices 1–4 in CI (no customer data) | S | TODO |

## Slice 1 evidence (2026-09-20)

- **Types**: `HierarchyInfo` / `HierarchyLevelInfo` in `tools/tabular_model.rs`;
  `parse_hierarchy_levels` shared by the BIM and folder parsers; the TMDL parser
  reads `level` children (`column`, `ordinal`).
- **Converter**: `emit_hierarchy` picks the first hierarchy with resolvable
  levels (the runtime config models one hierarchy per dimension), emits
  `hierarchy_levels` with source columns and `level_number`, and sets
  `hierarchy_name` to the export name so unique names match
  (`[Dates].[Calendar Hierarchy].[year]`). Levels whose column is missing from
  `schema.sql` are skipped and reported.
- **Bug fixed**: relationship endpoints resolve through the table's columns
  (`Customer ID` → `customerid`), the same mapping hierarchy levels use.
- **Report**: a "Hierarchies" section lists every declared hierarchy with its
  levels and status (emitted / partial / not emitted), and the summary counts
  them.
- **Verified live** against the retail sample in all three formats:
  `MDSCHEMA_HIERARCHIES` / `MDSCHEMA_LEVELS` expose
  `[Dates].[Calendar Hierarchy]` with `year → quartername → monthname →
  fulldate`; a year slicer returns data; `DrilldownLevel({All})` expands to
  2020–2030; `DrilldownMember(2022)` nests Q1–Q4.
- **Verified locally** against the real export: 6 role-playing calendars with
  23 levels emitted, 0 of 16 relationship endpoints missing from `schema.sql`.
- 450 tests green, clippy/fmt clean.

## Non-goals

- Multiple hierarchies per table (the config carries one per dimension; extras
  are reported, not emitted — derived dimensions are a possible follow-up).
- Hierarchy captions/display folders (the export name is used).
- DAX coverage (frozen by plan 044).

## Done criteria

- A real export converts with correct joins and drill paths; `qualify` names
  any relationship semantics it cannot honour instead of silently mis-joining.
- All date roles carry levels and time intelligence.
- The gap-model fixture covers slices 1–4 in CI.
