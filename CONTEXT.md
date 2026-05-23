# SSAS Proxy — Session Context

## Goal
Rust proxy that impersonates an SSAS server to satisfy Excel's MSOLAP client.
Eventually: transpile MDX → Malloy → DuckDB.
**Current status:** Excel can place `Produktkategori` and `Region` on Rows,
both together or independently, filter by either dimension, and add
`Total Försäljning` to Values. All three cube dimensions (Measures,
Produktkategori, Region) work correctly through the full metadata→probe→query
cycle with real data from the SQLite backend. 43 unit tests.

## Recent fixes (2026-05-23)

### Two-visible-hierarchy CrossJoin support
- Added `axis_dimensions: Vec<String>` to `SemanticQuery` — tracks all
  visible axis hierarchies in order (1 for simple drilldown, 2 for CrossJoin).
- Added `parse_axis_dimensions()` — extracts dimensions from the SELECT part
  before `FROM [Model]`.
- Added `build_drilldown_multi()` — builds Axis0 with two hierarchies and
  cross-product tuples (e.g. `(Kategori A, North)`, `(Kategori A, South)`).
- `full_slicer_axis()` skips all axis dimensions (not just `row_dimension`).
- Backend: `grouped_pairs()` returns `Vec<(String, String, f64)>` for
  `Produktkategori x Region` grouped queries.

### CrossJoin SlicerAxis bug fix
- `SlicerAllAndMeasure` was matching drilldown queries that had
  `WHERE ([Region].[Region].[All],[Measures]...)` even when the query
  also had `DrilldownLevel(...)` and `ON COLUMNS`.
- Moved `SlicerAllAndMeasure` below `is_drilldown` and gated with `!has_axes`.
- This was the root cause of "Total Försäljning won't go into Values."

### Complete SlicerAxis (all off-axis dimensions)
- `full_slicer_axis()` now includes EVERY cube dimension not on the visible
  axis, in stable metadata-ordinal order (Measures, Produktkategori, Region).
- Default All member emitted even when a dimension is not referenced in WHERE.
- Off-axis slicer members use standard 5 properties only (not row-axis dim_props).

### Dimension-tagged filter model
- Replaced flat `category_filters: Vec<String>` with
  `filters: Vec<DimensionFilter>` (dimension + members).
- Replaced `dimension: Option<String>` with `row_dimension: Option<String>`.
- Added `SlicerSelection` for off-axis dimensions in WHERE clause.
- `parse_mdx_filters()` returns dimension-tagged filters instead of flat list.
- Filter parsing is clause-aware: WHERE content extracted with balanced parens,
  subquery `SELECT ({...})` parsed separately.

### Multi-category subquery filter fix
- Replaced `parse_category_filter()` (global string matching) with:
  - `where_clause_payload()` (balanced-paren scanning)
  - `extract_dimension_member_names()` (per-dimension member extraction)
- Multi-category subquery filters now return all categories, not just the first.

### Region dimension added
- Backend: added `region` column to `faktatabell`, 8 demo rows covering
  North/South × Kategori A-D.
- Metadata: dimensions, hierarchies, levels, members, tables,
  measuregroup_dimensions, mdschema_properties — all with Region entries.
- Region-aware queries: `sales_by_region()`, `sales_for_regions()`,
  `total_for_regions()`, `region_count()`.
- Combined queries: `grouped_by_produktkategori(region_filter)`,
  `grouped_by_region(kat_filter)`, `total_with_filters(region, kat)`.

### Flat rowset routing fix
- `is_mdx_select()` now matches `WITH ... SELECT ...` patterns.
- `WITH MEMBER [Measures].cChildren ...` probes now reach the cellset path.

### FilteredMembers parser fix
- `cchildren_target_is_measures()`, `cchildren_target_is_product_leaf()`,
  `cchildren_filtered_member_name()` — skip past the opening quote before
  searching for the closing quote.

### PARENT_UNIQUE_NAME omission for All member
- `produktkategori_dim_props_all()` no longer emits `PARENT_UNIQUE_NAME`.

### Debug file logging
- `debug-last-run.log` created fresh on every `cargo run`.
- Logs full Execute request/response XML and MDSCHEMA_MEMBERS response XML.

## Project structure
```
xmla_proxy/src/
  main.rs              — Router, dispatch, debug file logging, headers
  parser.rs            — parse_xmla() → XmlaRequest enum (quick-xml)
  response.rs          — wrap_in_soap_envelope(), discover_rowset_envelope(), UUID_TYPE
  properties.rs        — 14-property registry, filter-based DISCOVER_PROPERTIES
  schema_rowsets.rs    — DISCOVER_SCHEMA_ROWSETS (~60 entries)
  catalogs.rs          — DBSCHEMA_CATALOGS
  cubes.rs             — MDSCHEMA_CUBES
  tables.rs            — DBSCHEMA_TABLES (Faktatabell, Produktkategori, Region)
  dimensions.rs        — MDSCHEMA_DIMENSIONS (Measures hidden, Produktkategori, Region)
  hierarchies.rs       — MDSCHEMA_HIERARCHIES ([Measures], [Produktkategori], [Region])
  levels.rs            — MDSCHEMA_LEVELS (MeasuresLevel, Prod.All/Prod, Region.All/Region)
  measures.rs          — MDSCHEMA_MEASURES (Total Försäljning)
  measure_groups.rs    — MDSCHEMA_MEASUREGROUPS (Faktatabell)
  measuregroup_dimensions.rs — MDSCHEMA_MEASUREGROUP_DIMENSIONS
  members.rs           — MDSCHEMA_MEMBERS (Produktkategori + Region members)
  mdschema_properties.rs — MDSCHEMA_PROPERTIES (both dimensions)
  literals.rs          — DISCOVER_LITERALS
  sets.rs              — MDSCHEMA_SETS (empty)
  kpis.rs              — MDSCHEMA_KPIS (empty)
  tmschema.rs          — TMSCHEMA_* stubs
  execute.rs           — Thin dispatch (42 lines) + 43 unit tests
  execute_builders.rs  — Cellset response builders; all build_* functions,
                         full_slicer_axis, multi-hierarchy Axis0 builder
  mdx_semantic.rs      — MDX parsing, extraction, semantic classification
                         (SemanticQueryKind, SemanticQuery, DimensionFilter,
                         SlicerSelection, parse_axis_dimensions)
  backend.rs           — SQLite backend (rusqlite): faktatabell with
                         produktkategori, region, sales; grouped/filtered queries
  cellset.rs           — Generic cellset XML builder (mddataset)
  rowset.rs            — Rowset infrastructure (currently unused)
```

## What works
- Full discover handshake; Excel reaches the PivotTable without issues.
- PivotTable Fields renders: `Σ Faktatabell`, `Total Försäljning (SEK)`,
  `Produktkategori`, `Region`.
- Session management (BeginSession, EndSession, empty Execute).
- `X-Transport-Caps-Negotiation-Flags: 0,0,0,0,0` header.
- **Single-dimension drilldown**: `DrilldownLevel` for Produktkategori or Region.
- **Two-dimension CrossJoin**: `Produktkategori` and `Region` both on Rows.
- **Cross-dimension filtering**:
  - Produktkategori on Rows + Region in Filter
  - Region on Rows + Produktkategori in Filter
  - All, single member, and switching between them
- **Slicer-only** `WHERE (...)` queries for both dimensions.
- **Probe queries**: `All.Members`, `All.Children`, `Leaf.Children`,
  `Measure.Children`, `cChildren + Ascendants(...)` for both dimensions.
- Filter dropdown opens and changes with real data.
- Debug logging to `debug-last-run.log` (resets each run).
- `MDSCHEMA_MEMBERS` filter logic is dimension-agnostic (PARENT/ANCESTORS use actual parent names, not hardcoded Produktkategori All).
- 43 unit tests in `execute.rs` (routing, parsing, classification, response shape, combined dimensions, multi-hierarchy).

## What does not yet work
- **Expand/collapse on 2-hierarchy axis**: Excel sends `DrilldownMember(CrossJoin(...), excluded_set, ...)` to collapse a category leaf. This query shape is not yet parsed or built.
- Full N-way MDX generalization — parsing is still substring-driven for the observed Excel subset.

## Next workstreams
1. **`nom`-based MDX parser.** Replace substring-driven parsing in
   `mdx_semantic.rs` with a proper parser. Keep the same `SemanticQuery`
   output type and existing 43 tests.
2. **`DrilldownMember` expand/collapse support.** Once `nom` is in place,
   add parsing and builder support for the collapse query shape Excel sends
   when collapsing a leaf node on a 2-hierarchy axis.
3. **ExecutionPlan layer.** Introduce a backend-neutral execution plan
   between `SemanticQuery` and the builder/backend.
4. **Malloy generation.** Generate Malloy from the execution plan.
5. **DuckDB backend.** Swap out SQLite once Malloy is stable.
6. **Vec<Row> refactor.** Apply the Rowset infrastructure to a simpler
   rowset (e.g. KPIS, SETS — empty) first.

## Key lessons learned (additions since last update)

13. **Excel uses `CrossJoin(DrilldownLevel(...), DrilldownLevel(...))` to
    place a second field on Rows.** The proxy must detect this shape and
    build a multi-hierarchy Axis0 with cross-product tuples. Responding
    with a single-hierarchy axis causes Excel to leave the field unchecked
    or change sheet data incorrectly.

14. **SlicerAxis must contain every off-axis cube dimension**, not just
    dimensions mentioned in the WHERE clause. Dimensions not referenced
    in WHERE still need their default All member on SlicerAxis. Order must
    be stable by metadata ordinal.

15. **Off-axis SlicerAxis members must use standard 5 properties only.**
    Inheriting row-axis dimension properties (PARENT_UNIQUE_NAME, MEMBER_KEY,
    etc.) on SlicerAxis members confuses Excel.

16. **Classification order matters.** `SlicerAllAndMeasure` must be gated
    behind drilldown/axis checks, otherwise a drilldown query with
    `WHERE (All, Measure)` is misclassified as slicer-only.

17. **`DrilldownMember(CrossJoin(...), excluded, hierarchy)` is the query
    shape Excel uses for 2-hierarchy expand/collapse.** Not yet supported.

## Hard-coded constants
- Catalog name: `KTH_KEX_MALLOY_CUBE`
- Cube name: `Model`
- Measure name: `Total Försäljning` (caption `Total Försäljning (SEK)`)
- Measure group: `Faktatabell`
- Dimensions: `Produktkategori`, `Region`
- Session ID: `RUST-SESSION-456` (in response.rs)
- Cube dimension ordinal order: `ALL_DIMS = ["Measures", "Produktkategori", "Region"]` (in execute_builders.rs)
