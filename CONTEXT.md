# SSAS Proxy — Session Context

## Goal
Rust proxy that impersonates an SSAS server to satisfy Excel's MSOLAP client.
Eventually: transpile MDX → Malloy → DuckDB.
Currently: get past the DISCOVER phase and reach the Excel cube browser.

## Project structure
```
xmla_proxy/src/
  main.rs              — Router (POST /xmla), handle_xmla dispatch, headers
  parser.rs            — parse_xmla() → XmlaRequest enum (quick-xml streaming)
  response.rs          — wrap_in_soap_envelope(), discover_rowset_envelope(), UUID_TYPE
  properties.rs        — 14-property registry, filter-based DISCOVER_PROPERTIES response
  schema_rowsets.rs    — 54 DISCOVER_SCHEMA_ROWSETS entries (full MSOLAP inventory)
  catalogs.rs          — DBSCHEMA_CATALOGS (1 catalog)
  cubes.rs             — MDSCHEMA_CUBES (1 cube: Model)
  tables.rs            — DBSCHEMA_TABLES (1 MEASURE_GROUP + 1 CUBE_DIMENSION + 10 $SYSTEM)
  dimensions.rs        — MDSCHEMA_DIMENSIONS (2 dims: Measures TYPE=2, Produktkategori TYPE=0)
  hierarchies.rs       — MDSCHEMA_HIERARCHIES (2 hierarchies, both STRUCTURE=3)
  levels.rs            — MDSCHEMA_LEVELS (2 levels, both LEVEL_TYPE=0)
  measures.rs          — MDSCHEMA_MEASURES (1 measure: Total Försäljning)
  members.rs           — MDSCHEMA_MEMBERS (2 members: Total Forsaljning + All Produktkategorier)
  mdschema_properties.rs — MDSCHEMA_PROPERTIES (3 member properties placeholder)
  execute.rs           — ExecuteResponse (empty, session, MDX statement)
```

## XmlaRequest variants (all handled)
DiscoverProperties(Vec<String>), DiscoverSchemaRowsets, DbSchemaCatalogs,
MdschemaCubes, DbschemaTables, MdschemaDimensions, MdschemaMeasures,
MdschemaHierarchies, MdschemaLevels, MdschemaProperties, MdschemaMembers,
BeginSession, ExecuteEmpty, ExecuteStatement(String), Unknown

## What works
- Full discover handshake: properties → schema rowsets → catalogs → cubes → tables → dimensions → hierarchies → levels → properties → members
- Session management (BeginSession, EndSession, empty Execute)
- X-Transport-Caps-Negotiation-Flags: 0,0,0,0,0 header
- MDX SELECT FROM [Model] returns a minimal rowset (Total_Forsaljning column)

## Current blocker
**Cube selection dialog won't close.** Excel completes the discover handshake through MDSCHEMA_LEVELS/MEMBERS, then sends EndSession without ever sending an MDX query. Clicking OK on the cube dialog does nothing — dialog stays open.

## Latest changes (not tested yet)
- Expanded tables.rs with $SYSTEM schema rows + MEASURE_GROUP row
- Added members.rs handler (MdschemaMembers)
- Hierarchies: STRUCTURE=3 (was 1) for Produktkategori
- Levels: LEVEL_TYPE=0 (was 1) for Measures

## Key protocol trace (from real SSAS, session 2)
```
DISCOVER_PROPERTIES (5 filtered)
DISCOVER_PROPERTIES (Catalog)
DISCOVER_PROPERTIES (6 server caps)
DISCOVER_SCHEMA_ROWSETS
MDSCHEMA_PROPERTIES (new!)
MDSCHEMA_CUBES ×3
DISCOVER_SCHEMA_ROWSETS (re-requested!)
MDSCHEMA_DIMENSIONS
MDSCHEMA_HIERARCHIES
MDSCHEMA_LEVELS
EndSession  ← never reaches MDX query phase
```

## Next steps to try
1. Verify expanded tables + members fix the dialog issue
2. If still stuck: check if Excel needs specific member properties (MEMBER_CAPTION, MEMBER_VALUE, etc.) in MDSCHEMA_PROPERTIES
3. If still stuck: compare our cube response column-by-column with real SSAS
4. Eventually: implement proper MDX → Malloy transpilation
