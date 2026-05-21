# SSAS Proxy — Session Context

## Goal
Rust proxy that impersonates an SSAS server to satisfy Excel's MSOLAP client.
Eventually: transpile MDX → Malloy → DuckDB.
**Current status:** Excel renders the PivotTable Fields panel correctly with `Σ Faktatabell` (containing `Total Försäljning (SEK)`) and `Produktkategori` (containing the `Produktkategori` hierarchy). Drag/drop into the pivot has not been verified end-to-end yet.

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
  members.rs           — MDSCHEMA_MEMBERS (2 rows for Produktkategori)
  mdschema_properties.rs — MDSCHEMA_PROPERTIES (3 placeholder member properties)
  literals.rs          — DISCOVER_LITERALS
  sets.rs              — MDSCHEMA_SETS (empty)
  kpis.rs              — MDSCHEMA_KPIS (empty)
  tmschema.rs          — TMSCHEMA_* stubs (advertised but Excel does not query them)
  execute.rs           — ExecuteResponse with MDX SELECT + DAX EVALUATE branches
```

## XmlaRequest variants (all handled in main.rs)
DiscoverProperties, DiscoverSchemaRowsets, DiscoverLiterals,
DbSchemaCatalogs, MdschemaCubes, DbschemaTables, MdschemaDimensions, MdschemaMeasures,
MdschemaHierarchies, MdschemaLevels, MdschemaProperties, MdschemaMembers,
MdschemaSets, MdschemaKpis, MdschemaMeasureGroups, MdschemaMeasureGroupDimensions,
TmschemaModel/Tables/Columns/Measures/Hierarchies/Levels/Relationships/Partitions,
DiscoverXmlMetadata, DiscoverCalcDependency,
BeginSession, ExecuteEmpty, ExecuteStatement(String), Unknown.

## What works
- Full discover handshake; Excel reaches the PivotTable without the previous cube-selection dialog blocker.
- PivotTable Fields renders correctly: `Σ Faktatabell` + `Total Försäljning (SEK)`,
  and `Produktkategori` table + `Produktkategori` hierarchy.
- Session management (BeginSession, EndSession, empty Execute).
- `X-Transport-Caps-Negotiation-Flags: 0,0,0,0,0` header.
- MDX `SELECT ... FROM [Model]` returns a minimal rowset.
- DAX `EVALUATE` falls through to a placeholder single-row response.
- Request logging: every Discover prints the `<RestrictionList>` and `<PropertyList>`
  inner XML to stdout, which makes it cheap to learn what Excel actually asks for.

## Observed Excel session pattern (current trace)
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

## Next candidate workstreams
1. **Test pivot drag-drop end to end.** Confirm MDX execute returns the placeholder
   number when a measure is dragged into Values.
2. **Audit other MDSCHEMA rowsets for missing GUID columns.** Add `HIERARCHY_GUID`,
   `LEVEL_GUID`, `MEASURE_GUID`, `MEMBER_GUID` row data to head off the next
   surprise. Schema columns are already declared; only row data is missing.
3. **Vec<Row> + restriction filter refactor.** Pre-req for sourcing rows from
   DuckDB. Each rowset module exposes `fn columns() -> &[Column]`,
   `fn rows() -> Vec<Row>`; a shared serializer in `response.rs` parses
   `<RestrictionList>` from the request and filters before emitting XML.
4. **Real MDX → Malloy → DuckDB transpilation** in `execute.rs`. Currently a
   constant placeholder. Touchstones: `EVALUATE`, `SELECT ... FROM [Model]`.
5. **Cleanup.** TMSCHEMA stubs are inert. `mdschema_properties.rs` placeholder for
   PROPERTY_TYPE=2 (cell properties) should be reviewed once we move to real
   queries.

## Hard-coded constants (search-and-replace targets when going live)
- Catalog name: `KTH_KEX_MALLOY_CUBE`
- Cube name: `Model`
- Measure name: `Total Försäljning` (caption `Total Försäljning (SEK)`)
- Measure group: `Faktatabell`
- Dimension: `Produktkategori`
- Session ID: `RUST-SESSION-456` (in response.rs)
- Placeholder MDX/DAX result: `1250000.5`
