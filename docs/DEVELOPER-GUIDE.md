# Developer Guide

How the SSAS Proxy works, module by module. For new developers.

## Startup

`src/main.rs` orchestrates startup:

1. Init debug logging to `debug-last-run.log`.
2. Load the proxy project: reads `PROXY_CONFIG` env var, parses
   `proxy-config.json`, loads the `.malloy` model file. Falls back to
   `project3/` (at repo root) if no config is set.
3. Init DuckDB backend: opens a file-based database when `db_path` is
   set, otherwise creates an in-memory demo database with synthetic data.
4. Optionally start the Malloy runtime if `MALLOY_RUNTIME=1`.
5. Start axum HTTP server on port 8080 at `POST /xmla`.

## Request lifecycle

```
 Excel MSOLAP POST /xmla
   -> XmlaRequest (parser.rs)
   -> handle_xmla dispatch (main.rs)
     |  Discover -> rowset XML from model (xmla/discover/*.rs)
     |  Execute  -> MDX statement
     v
 MDX string
   -> ParsedMdx (mdx/parser.rs, nom parser)
   -> SemanticQuery (mdx/semantic.rs)
   -> QueryPlan { Total | GroupBy | Count | Empty } (engine/plan.rs)
   -> SQL or Malloy compilation (engine/sql.rs or engine/malloy.rs)
   -> DuckDB execution (backend/mod.rs)
   -> QueryResult { Scalar | Grouped | Pairs | Count | Empty }
   -> Cellset XML rendering (execute/builders.rs + execute/axis_members.rs)
   -> SOAP envelope wrap (xmla/response.rs)
   -> HTTP response
```

### Key data types

| Type | Location | Purpose |
|------|----------|---------|
| `ParsedMdx` | `src/mdx/parser.rs` | Structured parse tree from MDX string |
| `SemanticQuery` | `src/mdx/semantic.rs` | Classified query: kind, dimensions, filters, excluded members |
| `QueryPlan` | `src/engine/plan.rs` | Backend-neutral: what to compute (Total, GroupBy, Count, Empty) |
| `QueryResult` | `src/engine/plan.rs` | Rows from DuckDB (Scalar, Grouped, Pairs, Count, Empty) |
| `DimId` / `MeasId` | `src/engine/plan.rs` | `String` type aliases for dimension/measure identifiers |
| `SemanticModel` | `src/engine/model.rs` | Canonical cube metadata: fact tables, dimensions, measures |
| `ProxyConfig` | `src/project/config.rs` | Serde struct for `proxy-config.json` deserialization |
| `XmlaRequest` | `src/xmla/parser.rs` | Parsed XMLA request type (Discover, Execute, etc.) |

## Module map

```
src/
  main.rs                        HTTP server, XMLA dispatch, startup
  lib.rs                         Module declarations and legacy re-exports

  project/                       Config loading and project lifecycle
    config.rs                    ProxyConfig and related serde structs
    project.rs                   ProxyProject singleton, SemanticModel build from config

  mdx/                           MDX protocol layer
    parser.rs                    Nom parser: ParsedMdx, MemberRef, DimRef
    semantic.rs                  Classification: SemanticQuery, SemanticQueryKind

  engine/                        Query planning and execution
    model.rs                     SemanticModel, DimensionDef, MeasureDef, FactTable
    plan.rs                      QueryPlan, QueryResult, plan_from_semantic, execute_plan
    sql.rs                       SQL emitter: sql_for_query_plan
    malloy.rs                    Malloy emitter: malloy_source_with_model_text
    normalize.rs                 plan_key normalization
    cache.rs                     PlanCache for compiled SQL

    # Malloy runtime (Node.js worker)
    malloy_compiler.rs           MalloyCompiler trait, CompileResult
    malloy_node.rs               One-shot Node.js compiler (spike)
    malloy_node_longlived.rs     Long-lived worker client with compile_ms timing

    parity.rs                    Direct SQL vs Malloy result parity tests
    timing.rs                    Timings struct, RuntimePath enum

  execute/                       Query execution and XML rendering
    dispatch.rs                  Statement routing: DAX, MDX SELECT, MDX probes
    builders.rs                  Cellset rendering (11 query kind handlers),
                                 Malloy runtime toggles, compiler/cache integration
    axis_members.rs              XML helpers: members, axes, slicer, measurement cells

  xmla/                          XMLA protocol layer
    parser.rs                    XMLA envelope parser: XmlaRequest enum
    response.rs                  SOAP envelope wrapper
    rowset.rs                    Flat rowset XML builder
    cellset.rs                   Cellset/axis/member config types
    properties.rs                Session/property discovery responses
    schema_rowsets.rs            Schema rowset discovery
    discover/                    Discover rowset responses
      catalogs.rs, cubes.rs, tables.rs
      dimensions.rs, hierarchies.rs, levels.rs
      measures.rs, members.rs
      sets.rs, kpis.rs, literals.rs
      mdschema_properties.rs
      measure_groups.rs, measuregroup_dimensions.rs
      tmschema.rs                Tabular metadata rowsets

  backend/                       Database backends
    mod.rs                       DuckDB backend, QueryBackend trait, demo data generation

  test_support/                  Shared test code
    fixtures.rs                  MDX test fixture constants

  bin/                           Standalone tools
    convert_tabular.rs           Tabular Editor .bim to Malloy + DuckDB converter
    inventory.rs                 Model inventory extractor
    seed_sql.rs                  Synthetic data SQL generator
```

## Request dispatch in detail

### Discover requests

| RequestType | Module | Output |
|---|---|---|
| `DISCOVER_PROPERTIES` | `xmla/properties.rs` | Session properties |
| `DISCOVER_SCHEMA_ROWSETS` | `xmla/schema_rowsets.rs` | Available rowsets |
| `DBSCHEMA_CATALOGS` | `xmla/discover/catalogs.rs` | Catalog name |
| `DBSCHEMA_TABLES` | `xmla/discover/tables.rs` | Table list |
| `MDSCHEMA_CUBES` | `xmla/discover/cubes.rs` | Cube metadata |
| `MDSCHEMA_DIMENSIONS` | `xmla/discover/dimensions.rs` | Dimensions from model |
| `MDSCHEMA_HIERARCHIES` | `xmla/discover/hierarchies.rs` | Hierarchies from model |
| `MDSCHEMA_LEVELS` | `xmla/discover/levels.rs` | Level definitions |
| `MDSCHEMA_MEASURES` | `xmla/discover/measures.rs` | Measures from model |
| `MDSCHEMA_MEMBERS` | `xmla/discover/members.rs` | Member values from DuckDB |
| `MDSCHEMA_PROPERTIES` | `xmla/discover/mdschema_properties.rs` | Dimension properties |
| `MDSCHEMA_MEASUREGROUPS` | `xmla/discover/measure_groups.rs` | Measure groups from FactTables |
| `MDSCHEMA_MEASUREGROUP_DIMENSIONS` | `xmla/discover/measuregroup_dimensions.rs` | Per-measure-group dim mapping |
| `TMSCHEMA_*` | `xmla/discover/tmschema.rs` | Tabular metadata for Power BI |

### Execute requests

| Statement type | Detector | Dispatcher | Builder |
|---|---|---|---|
| DAX (`EVALUATE`) | `is_dax()` | `get_execute_dax_response()` | Direct rowset |
| MDX SELECT | `is_mdx_select()` | `get_execute_cellset_response_timed_malloy()` | Cellset via semantic pipeline |
| MDX probe (WITH MEMBER etc.) | Fallthrough | `get_execute_mdx_response()` | Direct rowset or cellset per kind |

## Naming conventions

Three distinct name types flow through the proxy. Never conflate them.

| Concept | Config field | Runtime field | Example | Purpose |
|---------|-------------|---------------|---------|---------|
| Internal ID | `id` | `QueryPlan` fields, `plan_key` | `"Category"` | Stable key for routing, caching, lookups |
| Semantic name | `malloy_name` | `semantic_name` | `"category"` | Matches Malloy source field, DuckDB column |
| Excel label | `caption` | `caption` | `"Category"` | Human-readable, appears in PivotTable |

See `docs/naming-contract.md` for full rules.

## Where to start debugging

1. **Excel metadata issues** - Check `xmla/discover/` rowset handlers.
   All discover responses are generated from `SemanticModel`.

2. **Excel returns wrong/missing data** - Check the MDX parse pipeline:
   - `mdx/semantic.rs:` - is `SemanticQuery` classification correct?
   - `engine/plan.rs:` - is `QueryPlan` built correctly?
   - `engine/sql.rs:` - is the SQL correct for the plan?
   - Enable `debug-last-run.log` (auto-written) and check the generated SQL.

3. **Malloy compile fails** - Check stderr for compile errors (logged automatically).
   The proxy falls back to direct SQL, so Excel stays functional. Fix the Malloy
   source or the emitter in `engine/malloy.rs`.

4. **Add a new XMLA rowset** - Add a variant to `XmlaRequest` in `xmla/parser.rs`,
   add a dispatch arm in `main.rs`, create a handler in `xmla/discover/`.

5. **Add a new query kind** - Add a variant to `SemanticQueryKind`, handle it in
   `plan_from_semantic_with_model()` and in `dispatch()` in `execute/builders.rs`.

## Stable vs transitional

| Component | Status | Notes |
|---|---|---|
| `engine/model.rs` | Stable | Semantic model types, well-documented |
| `engine/plan.rs` | Stable | Query plan, plan construction, plan execution |
| `engine/sql.rs` | Stable | SQL emission from QueryPlan |
| `engine/normalize.rs` | Stable | Plan key normalization for caching |
| `engine/timing.rs` | Stable | Timing instrumentation |
| `engine/malloy.rs` | Stable | Malloy emission |
| `project/config.rs` | Stable | Config schema |
| `project/project.rs` | Stable | Project loader, model builder |
| `xmla/discover/*.rs` | Stable | Metadata rowset generation |
| `xmla/response.rs` | Stable | SOAP envelope wrapper |
| `xmla/cellset.rs` | Stable | Cellset/axis/member config types |
| `xmla/parser.rs` | Stable | XMLA request parser |
| `backend/mod.rs` | Stable | DuckDB backend |
| `mdx/parser.rs` | Stable-ish | Nom parser, covers known patterns |
| `mdx/semantic.rs` | Needs work | Some hardcoded dimension constants remain |
| `execute/builders.rs` | Needs cleanup | Too large, mixed responsibilities |
| `execute/axis_members.rs` | Needs cleanup | Heavy, some model-agnostic gaps |
| `src/main.rs` | Needs cleanup | Mixed concerns, large match statement |
| `src/lib.rs` | Transitional | Legacy flat re-exports should be phased out |
| `engine/malloy_node.rs` | Legacy | One-shot compiler (replaced by long-lived) |
| `bin/convert_tabular.rs` | Active | Tabular converter, growing feature set |

## Multi-fact semantics

When a project uses multiple fact tables (`fact_tables` in `proxy-config.json`):

- Each measure belongs to one fact table via `fact_table` field.
- Each dimension can be `shared: true` (visible across all facts) or scoped
  to one fact table via `fact_table: "fact_id"`.
- `SemanticModel::dim_is_compatible_with_measure()` checks whether a
  dimension can be used with a given measure.
- Unrelated dimension filters are silently ignored by `compatible_filters()`.
- Unrelated row dimensions still need unified rendering (known gap).

## Test strategy

```bash
cargo test --lib -- --test-threads=1
```

- Tests live alongside code in `#[cfg(test)] mod tests {}` blocks.
- `engine/parity.rs` - Direct SQL vs Malloy result parity.
- Worker-dependent tests require serialization (`--test-threads=1`).
- Shared test fixtures: `src/test_support/fixtures.rs`.
- `project/project.rs` - Config parsing and model building.
- `execute/dispatch.rs` - MDX parsing, classification, end-to-end responses.
- Benchmark: `cargo bench` runs `benches/pipeline.rs`.

## Environment variables

| Variable | Effect |
|---|---|
| `PROXY_CONFIG` | Path to `proxy-config.json` (default: `../project3/proxy-config.json`) |
| `MALLOY_RUNTIME` | Set to `1` to enable Malloy compile path |

## Tools

| Binary | Purpose |
|---|---|
| `cargo run` | Start the proxy server (default binary) |
| `cargo run --bin convert_tabular -- <src> <dest>` | Convert Tabular Editor folder to proxy project |
| `cargo run --bin inventory -- <src>` | Extract model inventory from Tabular Editor folder |
| `cargo run --bin seed_sql` | Generate DuckDB seed SQL from demo data generation |

## Appendix: Config reference

Every field in `proxy-config.json`, with descriptions and defaults.

### Top-level

| Field | Type | Default | Description |
|---|---|---|---|
| `catalog` | string | required | Excel-visible catalog name |
| `cube` | string | required | Cube name (MDX FROM clause) |
| `source_name` | string | required | Malloy source name (must match `.malloy`) |
| `table_name` | string | required | DuckDB table name (single-fact mode) |
| `dialect` | string | required | Backend dialect (`"duckdb"`) |
| `malloy_model_file` | string | required | Path to `.malloy` file, relative to config |
| `db_path` | string\|null | `null` | Path to DuckDB file, relative to config. `null` = demo mode with synthetic in-memory data |
| `fact_tables` | array | `[]` | Fact table definitions (multi-fact mode) |
| `dimensions` | array | required | Dimension definitions |
| `measures` | array | required | Measure definitions |

When `fact_tables` is empty, the proxy uses single-fact mode with `source_name`/`table_name`.
When `fact_tables` is non-empty, all measures must declare `fact_table`.

### FactTableConfig

| Field | Type | Description |
|---|---|---|
| `id` | string | Unique identifier, referenced by dimension/measure `fact_table` fields |
| `source_name` | string | Malloy source name for this fact table |
| `table_name` | string | DuckDB physical table name |
| `measure_group_name` | string | SSAS measure group name displayed to Excel |

### DimensionConfig

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | string | required | Internal identifier for QueryPlan/plan_key |
| `malloy_name` | string | required | Must match field name in `.malloy` source |
| `physical_field` | string | required | DuckDB column name (may include `table.column` syntax) |
| `caption` | string | required | Excel-visible label |
| `description` | string | `""` | Human-readable description |
| `hierarchy_name` | string | required | SSAS hierarchy name |
| `all_level_name` | string | required | Level name for `(All)` member |
| `leaf_level_name` | string | required | Level name for leaf members |
| `ordinal` | u32 | required | Sort order in Excel field list |
| `visible` | bool | required | Show in Excel field list |
| `has_all` | bool | required | Whether dimension has an All member |
| `cardinality_hint` | u32 | required | Cardinality hint for XMLA metadata |
| `fact_table` | string\|null | `null` | Bind to a specific fact table (multi-fact mode). `null` = primary fact table |
| `shared` | bool | `false` | If true, this dimension is compatible with all fact tables |

**Dimension scoping rules:**
- `shared: true` — dimension is compatible with all measures. Use for truly cross-fact dimensions (e.g. date).
- `fact_table: "sales"` — dimension belongs to one fact table. Filters from this dimension are only applied when the selected measure is from the same fact table.
- Neither — dimension uses the primary (first) fact table.

### MeasureConfig

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | string | required | Internal identifier for QueryPlan/plan_key |
| `fact_table` | string\|null | `null` | Which fact table this measure belongs to (multi-fact mode). Required when `fact_tables` is non-empty |
| `malloy_name` | string | required | Must match measure name in `.malloy` source |
| `physical_expr` | string | required | Malloy expression (e.g. `"revenue.sum()"`) |
| `sql_expr` | string | required | SQL fallback expression (e.g. `"SUM(revenue)"`) |
| `caption` | string | required | Excel-visible measure name |
| `display_name` | string | required | Longer Excel label |
| `description` | string | `""` | Human-readable description |
| `format_string` | string | required | Excel format string (e.g. `"#,##0.00"`) |
| `units` | string | required | Unit label (e.g. `"USD"`, `""`) |
| `ordinal` | u32 | required | Sort order in Excel field list |
| `visible` | bool | required | Show in Excel field list |
| `aggregator` | u32 | `1` | XMLA MEASURE_AGGREGATOR (1=sum) |
| `measure_group_name` | string | required | SSAS measure group name |
| `numeric_precision` | u16 | `18` | XMLA NUMERIC_PRECISION |
| `numeric_scale` | i16 | `2` | XMLA NUMERIC_SCALE |
| `expression` | string | `""` | Original DAX expression (informational) |

### db_path resolution

`db_path` is resolved relative to the directory containing `proxy-config.json`.
When `null` or omitted, the proxy creates an in-memory DuckDB with synthetic
data (20k rows of `sales_fact`).

When set, both the Rust backend and the Malloy JS worker open the same file.
The JS worker receives the resolved path via the `DUCKDB_PATH` env var.

