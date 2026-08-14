# Developer Guide

How the SSAS Proxy works, module by module. For new developers.

## Startup

`src/main.rs` orchestrates startup:

1. Init debug logging to `debug-last-run.log`.
2. Load the proxy project: reads `PROXY_CONFIG` env var, parses
   `proxy-config.json`. Falls back to
   `projects/project3/` (at repo root) if no config is set.
3. Init DuckDB backend: opens a file-based database when `db_path` is
   set, otherwise creates an in-memory demo database with synthetic data.
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
    -> ParsedMdx (mdx/parser.rs, nom parser, cube-agnostic)
    -> SemanticQuery (mdx/semantic.rs, sourced from ParsedMdx struct fields)
    -> QueryPlan { Total | GroupBy | Count | Empty } (engine/plan.rs)
    -> SQL (engine/sql.rs)
    -> DuckDB execution with fallback capability gates (backend/mod.rs)
    -> QueryResult { Scalar | Grouped | Pairs | Count | Empty }
    -> Cellset XML rendering (execute/render.rs + execute/axis_members.rs)
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
    sql.rs                       SQL emitter: sql_for_query_plan
    normalize.rs                 plan_key normalization
    timing.rs                    Timings struct, RuntimePath enum

  execute/                       Query execution and XML rendering
    dispatch.rs                  Statement routing (DAX, MDX SELECT, MDX probes) —
                                 production path routes via main.rs; this module's
                                 get_execute_statement_response is the test seam,
                                 plus most end-to-end tests
    runtime.rs                   Execution entry: backend injection, timing
                                 instrumentation
    render.rs                    Cellset XML rendering: dispatch_with_backend,
                                 11 query-kind handlers
    builders.rs                  Thin public entry points / re-exports over runtime
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

  xmla_trace.rs                  NDJSON trace capture (XMLA_TRACE=1)

  backend/                       Database backends
    mod.rs                       DuckDB backend, QueryBackend trait, demo data generation

  test_support/                  Shared test code
    fixtures.rs                  MDX test fixture constants

  bin/                           Thin wrappers over tools/ (one file per tool)

  tools/                         Tool implementations (single source of truth)
    convert_tabular.rs           Tabular .bim/TMDL/folder to proxy project converter
    tabular_model.rs             Shared conversion types, classify_dax
    parse_tmdl.rs, parse_bim.rs, parse_folder.rs  Tabular source parsers
    m_query.rs                   Power Query M partition parsing
    data_loader.rs               Load-script rendering from M partitions
    inventory.rs                 Model inventory extractor
    qualify.rs                   Readiness gate: READY / PARTIAL / BLOCKED
    trace_replay.rs              XMLA trace replay/compatibility validator
    load_replay.rs               Concurrent replay against a live endpoint
    extract_trace_mdx.rs         Extract unique ExecuteStatement MDX from traces
    seed_generated_db.rs         Generate synthetic data for generated_project
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
| MDX SELECT | `is_mdx_select()` | `get_execute_cellset_response_with_backend_and_context()` (main.rs -> `execute/runtime.rs`) | Cellset via semantic pipeline |
| MDX probe (WITH MEMBER etc.) | Fallthrough | `get_execute_mdx_response()` | Direct rowset or cellset per kind |

Note: `execute/dispatch.rs::get_execute_statement_response` is the test-only
seam used by the end-to-end test suite; production routing lives in
`main.rs::route_request`.

## Naming conventions

Three distinct name types flow through the proxy. Never conflate them.

| Concept | Config field | Runtime field | Example | Purpose |
|---------|-------------|---------------|---------|---------|
| Internal ID | `id` | `QueryPlan` fields, `plan_key` | `"Category"` | Stable key for routing, caching, lookups |
| Semantic name | `malloy_name` (deprecated) | `semantic_name` | `"category"` | Legacy field, kept for backward compat |
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

4. **Add a new XMLA rowset** - Add a variant to `XmlaRequest` in `xmla/parser.rs`,
   add a dispatch arm in `main.rs`, create a handler in `xmla/discover/`.

5. **Add a new query kind** - Add a variant to `SemanticQueryKind`, handle it in
   `plan_from_semantic_with_model()` and in `dispatch()` in `execute/builders.rs`.

## Stable vs transitional

| Component | Status | Notes |
|---|---|---|
| `engine/model.rs` | Stable | Semantic model types, FallbackCapability, DateDimDef, multi-fact support |
| `engine/plan.rs` | Stable | Query plan, plan construction, fallback execution with capability gates |
| `engine/sql.rs` | Stable | SQL emission from QueryPlan, date-dim subquery, relationship joins |
| `engine/normalize.rs` | Stable | Plan key normalization for caching |
| `engine/timing.rs` | Stable | Timing instrumentation |
| `project/config.rs` | Stable | Config schema: time_intelligence, fallback_capability, is_date_role |
| `project/project.rs` | Stable | Project loader, model builder, parse_fallback_capability |
| `xmla/discover/*.rs` | Stable | Metadata rowset generation from model |
| `xmla/response.rs` | Stable | SOAP envelope wrapper |
| `xmla/cellset.rs` | Stable | Cellset/axis/member config types |
| `xmla/parser.rs` | Stable | XMLA request parser |
| `xmla_trace.rs` | Stable | NDJSON trace capture for compatibility gate |
| `backend/mod.rs` | Stable | DuckDB backend, date_dim seeding |
| `mdx/parser.rs` | Stable | Nom parser, cube-agnostic, structural axis dimension detection |
| `mdx/semantic.rs` | Stable | Classification driven by ParsedMdx structural fields |
| `execute/dispatch.rs` | Stable | Statement routing test seam, compatibility gate tests |
| `execute/runtime.rs` | Stable | Execution entry, runtime-path selection, timing |
| `execute/render.rs` | Stable | Cellset rendering, 11 query-kind handlers |
| `execute/builders.rs` | Stable | Thin shim over runtime/render |
| `execute/axis_members.rs` | Needs cleanup | Heavy, some model-agnostic gaps |
| `src/main.rs` | Needs cleanup | Mixed concerns, large match statement |
| `src/lib.rs` | Transitional | Legacy flat re-exports should be phased out |
| `tools/convert_tabular.rs` | Active | Tabular converter: fact detection, date-role detection, time metadata, DAX classification |

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
cargo test --lib
```

- Tests live alongside code in `#[cfg(test)] mod tests {}` blocks.
- 293 tests covering MDX parsing, semantic classification, plan generation,
  SQL emission, metadata rowsets, multi-fact routing, end-to-end cellset
  rendering, Excel replay/oracle verification, time intelligence, security
  roles, and compatibility-gate assertions.
- Shared test fixtures: `src/test_support/fixtures.rs`.
- `project/project.rs` - Config parsing and model building.
- `execute/dispatch.rs` - MDX parsing, classification, end-to-end responses,
  compatibility gate tests.
- Benchmark: `cargo bench` runs `benches/pipeline.rs`.

## Environment variables

| Variable | Effect |
|---|---|
| `PROXY_CONFIG` | Path to `proxy-config.json` (default: `projects/project3/proxy-config.json`) |
| `XMLA_TRACE` | Set to `1` to write full request/response NDJSON to `xmla-trace.jsonl` |
| `BIND_ADDRESS` | Override listen address:port (default: `127.0.0.1:8080`) |

## Tools

| Command | Purpose |
|---|---|
| `cargo run --bin mallard` | Start the proxy server |
| `cargo run --bin mallard -- convert-tabular <src> <dest>` | Convert Tabular Editor folder to proxy project |
| `cargo run --bin mallard -- inventory <src>` | Extract model inventory from Tabular Editor folder |
| `cargo run --bin mallard -- qualify <config> [trace]` | Emit READY/PARTIAL/BLOCKED readiness verdict |
| `cargo run --bin mallard -- trace-replay [trace.jsonl] [--project config.json]` | Replay captured XMLA trace and diff responses |
| `cargo run --bin mallard -- extract-trace [trace.jsonl]` | Extract unique ExecuteStatement MDX from trace as Rust consts |
| `cargo run --bin mallard -- load-replay [args...]` | Concurrently replay captured requests against a live /xmla endpoint |
| `cargo run --bin mallard -- seed-generated-db` | Seed generated_project DuckDB file with synthetic data |
| `cargo run --bin mallard -- seed-sql` | Emit SQL to create demo fact tables |

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
| `relationships` | array | `[]` | Dimension-to-fact table relationship definitions |
| `time_intelligence` | object\|null | `null` | Global time-intelligence configuration (date_dimension block) |
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
| `is_date_role` | bool | `false` | Marks this dimension as a date-role (calendar) dimension for time intelligence |

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
| `sql_fallback_file` | string\|null | `null` | Path to DuckDB SQL fallback file (complex measures) |
| `fallback_capability` | string\|null | `null` | Shape capability: `"ScalarOnly"`, `"Universal"`, or `null` (auto-detect) |
| `time_intelligence` | object\|null | `null` | Per-measure time intelligence config: `{"dimension_id", "flag_column"}` |

### db_path resolution

`db_path` is resolved relative to the directory containing `proxy-config.json`.
When `null` or omitted, the proxy creates an in-memory DuckDB with synthetic
data (20k rows of `sales_fact`).

When set, both the Rust backend and the Malloy JS worker open the same file.
The JS worker receives the resolved path via the `DUCKDB_PATH` env var.

### TimeIntelligenceConfig

Top-level `time_intelligence.date_dimension` block:

| Field | Type | Description |
|---|---|---|
| `dimension_id` | string | Which dimension serves as the calendar/date dimension |
| `table_name` | string | DuckDB date dimension table name (default: `"date_dim"`) |
| `date_key_column` | string | Date-key column joining to fact table (default: `"date_key"`) |
| `full_date_column` | string | Full DATE-type column (default: `"full_date"`) |
| `flag_columns` | object | Flag column names for period detection |
| `flag_columns.year_column` | string | Year column (default: `"year"`) |
| `flag_columns.quarter_column` | string | Quarter column (default: `"quarter"`) |
| `flag_columns.month_column` | string | Month column (default: `"month"`) |
| `flag_columns.ytd_flag_column` | string | YTD flag (default: `"ytd_flag"`) |
| `flag_columns.prior_year_ytd_flag_column` | string | Prior-year YTD flag (default: `"prior_year_ytd_flag"`) |
| `flag_columns.current_year_flag_column` | string | Current-year flag (default: `"current_year_flag"`) |
| `flag_columns.qtd_flag_column` | string | Quarter-to-date flag (default: `"qtd_flag"`) |
| `flag_columns.mtd_flag_column` | string | Month-to-date flag (default: `"mtd_flag"`) |

Per-measure `time_intelligence` block:

| Field | Type | Description |
|---|---|---|
| `dimension_id` | string\|null | Which date-role dimension this measure binds to |
| `flag_column` | string | Which flag column to filter on (e.g. `"ytd_flag"`) |

