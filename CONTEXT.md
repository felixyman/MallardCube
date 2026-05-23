# SSAS Proxy — Session Context

## Goal
Rust proxy that impersonates an SSAS server to satisfy Excel's MSOLAP client.
Eventually: transpile MDX → Malloy → DuckDB.
**Current status:** Excel renders the PivotTable Fields panel, can drag
`Produktkategori` to Rows and Filters, the filter dropdown works, and changing
the selected filter category now returns correct real-data values. The proxy
survives the full metadata→probe→query cycle without errors, including the
previously-crashing `cChildren + Ascendants(...)` leaf-member probe.

## Recent fixes (most recent first)
### `cChildren + Ascendants(...)` leaf probe works (2026-05-23)
- Removed `PARENT_UNIQUE_NAME` from `produktkategori_dim_props_all()` —
  the All member must omit this property entirely even when declared in
  HierarchyInfo (matches real SSAS behavior).
- Fixed `cchildren_target_is_measures()`, `cchildren_target_is_product_leaf()`,
  and `cchildren_filtered_member_name()` — the `FilteredMembers As '...'`
  parser was finding the opening quote instead of the closing quote, so
  leaf probes were misclassified or returned empty member names.
- Fixed `is_mdx_select()` — `WITH MEMBER ...` MDX fell through to the
  flat rowset path because `is_mdx_select` only checked for `SELECT` prefix.
  Now also matches `WITH ... SELECT ...` patterns.
- 23 unit tests added in `execute.rs` covering: routing, parsing
  (dimension props, cell props, filters, cChildren helpers), semantic
  classification (all known probe families), and response-shape assertions
  for the fragile `cChildren + Ascendants` probe.

### Real data backend (2026-05-23)
- `src/backend.rs` with in-memory SQLite (`rusqlite`): table `faktatabell`
  with `produktkategori` + `sales`, demo rows for Kategori A/B/C/D.
- `Execute` now uses `Backend` for totals and grouped values instead of
  hard-coded `1250000.5`.

### Known limitation
- `parse_category_filter` matches `[Produktkategori]` anywhere in the MDX
  string (not just the WHERE clause), so multi-category subquery filter
  extraction only captures the first category. Not yet blocking Excel
  functionality but needs fixing before full multi-filter support.

## Project structure
```
xmla_proxy/src/
  main.rs              — Router (POST /xmla), handle_xmla dispatch, headers,
                         log_discover_context (RestrictionList + PropertyList logging)
  parser.rs            — parse_xmla() → XmlaRequest enum (quick-xml streaming)
  response.rs          — wrap_in_soap_envelope(), discover_rowset_envelope(), UUID_TYPE
  properties.rs        — 14-property registry, filter-based DISCOVER_PROPERTIES response
  schema_rowsets.rs    — DISCOVER_SCHEMA_ROWSETS (~60 entries incl. TMSCHEMA_* family)
  catalogs.rs          — DBSCHEMA_CATALOGS (1 catalog: KTH_KEX_MALLOY_CUBE)
  cubes.rs             — MDSCHEMA_CUBES (1 cube: Model, CUBE_SOURCE=2, PREFERRED_QUERY_PATTERNS=1)
  tables.rs            — DBSCHEMA_TABLES (Faktatabell SYSTEM TABLE / MEASURE_GROUP +
                         Produktkategori TABLE / CUBE_DIMENSION)
  dimensions.rs        — MDSCHEMA_DIMENSIONS (2 dims: Measures TYPE=2 hidden,
                         Produktkategori TYPE=3 visible) — includes DIMENSION_GUID,
                         DIMENSION_MASTER_UNIQUE_NAME, CUBE_SOURCE
  hierarchies.rs       — MDSCHEMA_HIERARCHIES (1 hierarchy: [Produktkategori].[Produktkategori])
  levels.rs            — MDSCHEMA_LEVELS (2 levels: (All) hidden + Produktkategori visible)
  measures.rs          — MDSCHEMA_MEASURES (1 measure: Total Försäljning, with DAX EXPRESSION)
  measure_groups.rs    — MDSCHEMA_MEASUREGROUPS (Faktatabell)
  measuregroup_dimensions.rs — MDSCHEMA_MEASUREGROUP_DIMENSIONS
                         (Faktatabell↔[Measures], Faktatabell↔[Produktkategori])
  members.rs           — MDSCHEMA_MEMBERS (static XML output, typed filter logic)
                         TREE_OP per spec: 0x01=CHILDREN, 0x04=PARENT, 0x08=SELF, 0x20=ANCESTORS
  mdschema_properties.rs — MDSCHEMA_PROPERTIES (PROPERTY_TYPE 1/2/5 with correct
                         HierarchyInfo-qualified property names and LEVEL_UNIQUE_NAME)
  literals.rs          — DISCOVER_LITERALS
  sets.rs              — MDSCHEMA_SETS (empty)
  kpis.rs              — MDSCHEMA_KPIS (empty)
  tmschema.rs          — TMSCHEMA_* stubs (advertised but Excel does not query them)
  execute.rs           — Dispatch + semantic query layer (SemanticQueryKind,
                         SemanticQuery) + MDX parsing/extraction helpers +
                         cellset response builders. 23 unit tests.
  backend.rs           — SQLite demo backend (rusqlite): faktatabell schema,
                         category sales data, filtered totals.
  cellset.rs           — Generic cellset XML builder (MemberConfig, TupleConfig,
                         HierarchyConfig, AxisConfig, CellConfig, CellsetResponse).
                         Supports multi-member tuples, multi-hierarchy axes, and
                         conditional cell property emission.
  rowset.rs            — Typed rowset infrastructure (Row, ColumnDef, Rowset).
                         Built for Vec<Row> refactor; currently unused (members.rs
                         reverted to static XML because serializer format triggers
                         MSOLAP crash when schema column order differs).
```

## Reference docs
  docs/cellset-reference.md   — Exact cellset XML format, property declarations,
                                 common crash errors, and generation recipe.

## XmlaRequest variants (all handled in main.rs)
DiscoverProperties, DiscoverSchemaRowsets, DiscoverLiterals,
DbSchemaCatalogs, MdschemaCubes, DbschemaTables, MdschemaDimensions, MdschemaMeasures,
MdschemaHierarchies, MdschemaLevels, MdschemaProperties, MdschemaMembers,
MdschemaSets, MdschemaKpis, MdschemaMeasureGroups, MdschemaMeasureGroupDimensions,
TmschemaModel/Tables/Columns/Measures/Hierarchies/Levels/Relationships/Partitions,
DiscoverXmlMetadata, DiscoverCalcDependency,
BeginSession, ExecuteEmpty, ExecuteStatement(String), Unknown.

## What works
- Full discover handshake; Excel reaches the PivotTable without issues.
- PivotTable Fields renders correctly: `Σ Faktatabell` + `Total Försäljning (SEK)`,
  and `Produktkategori` table + `Produktkategori` hierarchy.
- Session management (BeginSession, EndSession, empty Execute).
- `X-Transport-Caps-Negotiation-Flags: 0,0,0,0,0` header.
- MDX `SELECT ... FROM [Model]` flat rowset returns a real total.
- DAX `EVALUATE` falls through to a placeholder single-row response.
- **Cellset (mddataset) responses** for all observed Excel MDX probe families:
  `All.Members`, `All.Children`, `Level.Children`, `Leaf.Children`,
  `Measure.Children`, `DrilldownLevel`, `cChildren + Ascendants(...)`,
  slicer-only `WHERE (...)`, and `WHERE (All, Measure)`.
- Filter dropdown opens and changing the selected category updates the
  pivot with real data from the SQLite backend.
- Request logging: every Discover prints the `<RestrictionList>` and `<PropertyList>`
  inner XML to stdout.
- `MDSCHEMA_MEMBERS` follows the spec column order and no longer crashes Excel
  when the filter dropdown is opened.
- 23 unit tests in `execute.rs` (routing, parsing, classification, response shape).

## Current blocker
**None.** The XMLA/protocol side and the filter/pivot cycle are now working
end-to-end with real data. The next step is structural cleanup before adding
new features.

## Next candidate workstreams
1. **Module split.** `execute.rs` (770 lines) does 4 jobs: dispatch, MDX
   parsing/extraction, semantic classification, and cellset construction.
   Split into `mdx_semantic.rs` (parsing + classification) and
   `execute_builders.rs` (cellset builders), keeping `execute.rs` for
   dispatch only.
2. **`nom`-based MDX parser.** Replace substring-driven parsing inside
   `semantic_query_from_mdx()` with a proper parser. Keep the same
   `SemanticQuery` output type and the same 23 tests to prevent regressions.
3. **Malloy generation.** Generate Malloy from the semantic query model
   rather than from raw MDX strings.
4. **DuckDB backend.** Swap out the SQLite demo backend once Malloy
   generation is stable.
5. **Vec<Row> refactor — next safe module.** The Rowset infrastructure exists
   and compiles. Apply it to a simpler rowset (e.g. KPIS, SETS — empty
   rowsets) first to validate the serializer incrementally.
```
Session 1 (probe):
  DISCOVER_PROPERTIES(filtered) ×3
  DISCOVER_SCHEMA_ROWSETS
  DBSCHEMA_CATALOGS
  MDSCHEMA_CUBES
  DBSCHEMA_TABLES
  EndSession

Session 2 (real):
  DISCOVER_PROPERTIES ×3
  DISCOVER_SCHEMA_ROWSETS
  MDSCHEMA_PROPERTIES (PROPERTY_TYPE=2 → MDPROP_CELL)
  MDSCHEMA_CUBES ×2
  DISCOVER_SCHEMA_ROWSETS (SchemaName=MDSCHEMA_HIERARCHIES)  ← support probe
  MDSCHEMA_CUBES
  MDSCHEMA_DIMENSIONS
  MDSCHEMA_HIERARCHIES
  MDSCHEMA_LEVELS
  DISCOVER_SCHEMA_ROWSETS (SchemaName=MDSCHEMA_MEASURES)  ← support probe
  MDSCHEMA_MEASURES
  DISCOVER_LITERALS  (Format=Tabular = flat rowset format, NOT Tabular model)
  MDSCHEMA_SETS
  MDSCHEMA_KPIS
  MDSCHEMA_CUBES
  MDSCHEMA_MEASUREGROUPS
  MDSCHEMA_MEASUREGROUP_DIMENSIONS
  → PivotTable Fields panel renders
```

Excel **never** queries (in this flow): MDSCHEMA_MEMBERS, DBSCHEMA_COLUMNS, TMSCHEMA_*,
DISCOVER_XML_METADATA. Restrictions are minimal — `CATALOG_NAME` + `CUBE_NAME` at most.
No `DIMENSION_VISIBILITY=1` filter — Excel takes whatever rows we return.

## Key lessons learned
1. **Excel ignores `*_IS_VISIBLE` flags in the field-list pane.** Hiding a system dim
   requires either removing it entirely from MDSCHEMA_DIMENSIONS or providing the
   right identifying columns so Excel's own special-case logic kicks in.
2. **`DIMENSION_GUID` and `DIMENSION_MASTER_UNIQUE_NAME` are de-facto required**
   on every MDSCHEMA_DIMENSIONS row, even though the spec marks them optional.
   Without them, captions are suppressed AND the system [Measures] dim leaks
   into the field list as an empty unnamed node.
3. **Excel stays in MDX/multidim mode** even with `CUBE_SOURCE=2` and
   `PREFERRED_QUERY_PATTERNS=1`. `DbpropMsmdMDXCompatibility=1` in the request
   property list confirms it. TMSCHEMA_* rowsets are never queried.
4. **`DISCOVER_LITERALS Format=Tabular` is SOAP rowset format**, not Tabular model.
5. **Restriction filtering is not what blocked rendering** — leaving it for later
   when sourcing live data.
6. **`DEFAULT_HIERARCHY` must resolve to an existing hierarchy.** When we removed
   the `[Measures]` hierarchy from `MDSCHEMA_HIERARCHIES` while keeping the
   dimension with `DEFAULT_HIERARCHY=[Measures]`, Excel refused to issue MDX
   queries because the cube model was internally inconsistent.
7. **Cellset response (mddataset) format is non-negotiable for multidimensional MDX.**
   Flat rowset responses are rejected by Excel for queries with DIMENSION PROPERTIES
   or CELL PROPERTIES clauses. The correct format is documented in
   `docs/cellset-reference.md`.
8. **Every Guild column in every rowset schema must have row data**, even though
   the MS-SSAS spec marks them all as `minOccurs="0"`. Missing GUIDs cause silent
   rejection: empty/unnamed nodes (DIMENSION_GUID), cube validation failure
   (CUBE_GUID), and MDX refusal (HIERARCHY_GUID, LEVEL_GUID, MEASURE_GUID).
9. **MDSCHEMA_PROPERTIES must return HIERARCHY_UNIQUE_NAME on every row** when
   Excel filters by hierarchy. Without it, Excel sees zero matching properties
   and silently aborts the MDX phase.
10. **Metadata consistency is paramount.** Every reference must resolve:
    `DEFAULT_HIERARCHY` → actual hierarchy row, level references in
    MDSCHEMA_PROPERTIES → actual level rows, `DIMENSION_UNIQUE_NAME` references
    in member data → actual dimension rows. Excel silently rejects the cube
    when references are dangling.
11. **MSOLAP's rowset parser is extremely sensitive to column order and layout.**
    The `Rowset::to_xml()` serializer (rowset.rs) produces valid XML that differs
    from the old hand-written strings only in edge-case formatting. Even minor
    differences corrupt MSOLAP's internal state (crash on next unrelated request).
    The safe path: keep hand-written static XML for output, add filtering logic
    on top of the static strings. The Rowset infrastructure remains for future
    modules where format sensitivity can be re-tested incrementally.
12. **Cellset (mddataset) HierarchyInfo child elements are property DECLARATIONS,**
    not data. They use `name` and `type` attributes to declare what properties
    each Member element will carry. Every property that appears as a child of
    `<Member>` must be pre-declared in the corresponding HierarchyInfo, and
    undeclared member children cause `MDDSAxis::MoveToHierProperty` crashes.
    Duplicate qualified names between standard 5 (UName/Caption/LName/LNum/DisplayInfo)
    and intrinsic properties (MEMBER_UNIQUE_NAME/MEMBER_CAPTION/LEVEL_NUMBER/LEVEL_UNIQUE_NAME)
    are NOT allowed — the standard 5 cover these.

## Next candidate workstreams
1. **Semantic query layer.** Replace `execute.rs` substring-driven builders with a
   semantic model for the current Excel MDX subset:
   `DrilldownLevel`, `Level.Members`, `Member.Children`, slicer-only `WHERE (...)`,
   and the `cChildren + Ascendants(...)` probe.
2. **DuckDB demo backend.** Execute the semantic query model against a single narrow
   schema (`produktkategori`, `sales`) so filters and grouped totals return real data.
3. **Malloy generation.** Once the semantic layer is stable, generate Malloy from the
   semantic query model rather than from raw MDX strings.
4. **Vec<Row> refactor — next safe module.** The Rowset infrastructure exists and
   compiles. Apply it to a simpler rowset (e.g. KPIS, SETS — empty rowsets)
   first to validate the serializer incrementally.

## Hard-coded constants (search-and-replace targets when going live)
- Catalog name: `KTH_KEX_MALLOY_CUBE`
- Cube name: `Model`
- Measure name: `Total Försäljning` (caption `Total Försäljning (SEK)`)
- Measure group: `Faktatabell`
- Dimension: `Produktkategori`
- Session ID: `RUST-SESSION-456` (in response.rs)
- Placeholder MDX/DAX result: `1250000.5`
