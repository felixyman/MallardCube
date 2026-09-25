# Developer Guide

How the SSAS Proxy works, module by module. For new developers.

> **Boundary first:** the proxy is a protocol adapter, not a semantic layer.
> Before adding a feature that puts measure logic in the proxy, read
> [`DESIGN-INVARIANTS.md`](DESIGN-INVARIANTS.md) — the invariants and the
> upstream recipe are enforced by `qualify --strict`.

## Startup

`src/main.rs` orchestrates startup:

1. Init debug logging to `debug-last-run.log`.
2. Load the proxy project: reads `PROXY_CONFIG` env var, parses
   `proxy-config.json`. Falls back to
   `projects/project3/` (at repo root) if no config is set.
3. Init DuckDB backend: opens a file-based database when `db_path` is
   set, otherwise seeds a temporary demo database file with synthetic data.
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
    config_io.rs                 JSON/YAML load, section files, canonical writer
    project.rs                   ProxyProject singleton, SemanticModel build from config

  mdx/                           MDX protocol layer
    parser.rs                    Nom parser: ParsedMdx, MemberRef, DimRef
    semantic.rs                  Classification: SemanticQuery, SemanticQueryKind

  engine/                        Query planning and execution
    model.rs                     SemanticModel, DimensionDef, MeasureDef, FactTable
    plan.rs                      QueryPlan, QueryResult, plan_from_semantic, execute_plan
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
    cache.rs                     Short-lived QueryResult cache (plan 032):
                                 collapses Excel's per-CELL-PROPERTIES repeats
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

  status.rs                      `/health` + `/status` payloads and the
                                 data-freshness stamp (plan 041)
  reload.rs                      Data reload: file stamp, stale-sidecar check
                                 (plan 041 phase C)

  dim_cache.rs                   Per-dimension member dictionaries: All-member
                                 cardinality, leaf values, level paths
                                 (plan 031)

  xmla_trace.rs                  NDJSON trace capture (XMLA_TRACE=1)

  backend/                       Database backends
    mod.rs                       DuckDB backend, QueryBackend trait, demo data
                                 generation; `#[cfg(test)] test_fixture()` is the
                                 only demo backend — production always carries a
                                 BackendSource (file or temp-file demo)

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
| Excel label | `caption` | `caption` | `"Category"` | Human-readable, appears in PivotTable |
| DuckDB column | `physical_field` | `physical_field` | `"category"` | Physical column backing a dimension |

See `docs/naming-contract.md` for full rules.

## Where to start debugging

1. **Excel metadata issues** - Check `xmla/discover/` rowset handlers.
   All discover responses are generated from `SemanticModel`.

2. **Excel returns wrong/missing data** - Check the MDX parse pipeline:
   - `mdx/semantic.rs:` - is `SemanticQuery` classification correct?
   - `engine/plan.rs:` - is `QueryPlan` built correctly?
   - `engine/sql.rs:` - is the SQL correct for the plan?
   - Enable `debug-last-run.log` (auto-written) and check the generated SQL.

3. **Add a new XMLA rowset** - Add a variant to `XmlaRequest` in `xmla/parser.rs`,
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
| `execute/cache.rs` | Stable | Short-lived result cache (plan 032), user-scoped keys |
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
- 445 tests covering MDX parsing, semantic classification, plan generation,
  SQL emission, metadata rowsets, multi-fact routing, end-to-end cellset
  rendering, Excel replay/oracle verification, time intelligence, security
  roles, and compatibility-gate assertions.
- Shared test fixtures: `src/test_support/fixtures.rs`.
- Test-only backends: `Backend::test_fixture()` (a temp-file copy of the demo
  database) and `FileQueryBackend` in `execute/dispatch.rs` (a file-backed
  project's own DB). Parameterless helpers such as
  `dispatch::get_execute_statement_response` are `#[cfg(test)]` — production
  code always receives the request's backend, so no test seam can shadow real
  data.
- `project/project.rs` - Config parsing and model building.
- `execute/dispatch.rs` - MDX parsing, classification, end-to-end responses,
  compatibility gate tests.
- Benchmark: `cargo bench` runs `benches/pipeline.rs`.
- Load/scale harness: `scripts/bench.sh` (100M-row numbers in
  `docs/SCALING.md`); RLS rollup A/B: `scripts/rls-rollup-ab.sh`.

## Environment variables

| Variable | Effect |
|---|---|
| `PROXY_CONFIG` | Path to `proxy-config.json` (default: `projects/project3/proxy-config.json`) |
| `MALLARDCUBE_DB` | DuckDB file for AutoModel detection (overrides `PROXY_CONFIG`) |
| `MALLARDCUBE_FACT` | Fact table override for AutoModel |
| `MALLARDCUBE_POOL_SIZE` | Pooled read-only DuckDB connections (default: CPU count, capped 32) |
| `MALLARDCUBE_MEMORY_LIMIT` | Engine memory ceiling (`4GiB`, `4GB`, `4294967296B`, `80%`); default is 70% of the container's cgroup limit divided between the query slots, else the engine default |
| `MALLARDCUBE_MAX_CONCURRENT_QUERIES` | Requests allowed to run engine queries at once (default: CPU count / 4, at least 1). Bounds concurrency and, with it, the memory the per-slot ceiling adds up to |
| `MALLARDCUBE_QUERY_TIMEOUT_S` | Per-request engine timeout in seconds (default 300; `0` disables). On expiry the query is interrupted and the client gets a SOAP fault |
| `MALLARDCUBE_MAX_MEMBERS_PER_RESPONSE` | Member cap per response (default 1000000; `0` disables); over it the client gets a SOAP fault naming the limit |
| `MALLARDCUBE_MAX_CELLS` | Cell cap per cellset (default 2000000; `0` disables) |
| `MALLARDCUBE_MAX_RESPONSE_MB` | Whole-response byte cap in MB (default 512; `0` disables), covering every response shape |
| `MALLARDCUBE_TEMP_DIR` | Spill directory for large sorts/aggregations (created if missing); default is the engine's |
| `MALLARDCUBE_THREADS` | Engine thread count; default is all cores |
| `MALLARDCUBE_AGG_CACHE` | Aggregation sidecar path; enables rollups for SUM measures |
| `MALLARDCUBE_RESULT_CACHE` | Set to `0` to disable the 5 s result cache |
| `MALLARDCUBE_CACHE_MAX_BYTES` | Byte budget for the result cache (default 67108864; `0` keeps only the 64-entry cap) |
| `MALLARDCUBE_ALLOW_ANONYMOUS` | Set to `1` to serve on a non-loopback address without auth; without it a non-loopback bind is refused, because every reachable client would be an administrator |
| `MALLARDCUBE_RELOAD_WATCH` | Seconds between data-file stamp checks; a change triggers a reload |
| `XMLA_TRACE` | Set to `1` to write full request/response NDJSON to `xmla-trace.jsonl` |
| `MALLARDCUBE_DEBUG` | Set to `1` to write a verbose request log to `debug-last-run.log` |
| `BIND_ADDRESS` | Override listen address:port (default: `127.0.0.1:8080`) |

## Tools

| Command | Purpose |
|---|---|
| `cargo run --bin mallard` | Start the proxy server |
| `cargo run --bin mallard -- convert-tabular <src> <dest>` | Convert Tabular Editor folder to proxy project |
| `cargo run --bin mallard -- inventory <src>` | Extract model inventory from Tabular Editor folder |
| `cargo run --bin mallard -- qualify <config> [trace]` | Emit READY/PARTIAL/BLOCKED readiness verdict |
| `cargo run --bin mallard -- fmt [--check] [--to yaml\|json] <config>` | Canonicalize a JSON/YAML config, or print it in the other format |
| `cargo run --bin mallard -- trace-replay [trace.jsonl] [--project config.json]` | Replay captured XMLA trace and diff responses |
| `cargo run --bin mallard -- extract-trace [trace.jsonl]` | Extract unique ExecuteStatement MDX from trace as Rust consts |
| `cargo run --bin mallard -- load-replay [args...]` | Concurrently replay captured requests against a live /xmla endpoint |
| `cargo run --bin mallard -- seed-sql` | Emit SQL to create demo fact tables |

## Appendix: Config reference

Every field in `proxy-config.json`, with descriptions and defaults.

### Top-level

| Field | Type | Default | Description |
|---|---|---|---|
| `catalog` | string | required | Excel-visible catalog name |
| `cube` | string | required | Cube name (MDX FROM clause) |
| `source_name` | string | `table_name` | Legacy source name (informational; no longer consumed by the runtime) |
| `table_name` | string | `""` | DuckDB table name (single-fact mode) |
| `dialect` | string | `"duckdb"` | Backend dialect |
| `db_path` | string\|null | `null` | Path to DuckDB file, relative to config. `null` = demo mode with a temporary synthetic-data file |
| `fact_tables` | array | `[]` | Fact table definitions (multi-fact mode) |
| `relationships` | array | `[]` | Dimension-to-fact table relationship definitions |
| `time_intelligence` | object\|null | `null` | Global time-intelligence configuration (date_dimension block) |
| `dimensions` | array | `[]` | Dimension definitions |
| `measures` | array | `[]` | Measure definitions |
| `dimensions_file` | string\|null | `null` | Section file holding dimensions (merged after inline entries; plan 040) |
| `measures_file` | string\|null | `null` | Section file holding measures |
| `relationships_file` | string\|null | `null` | Section file holding relationships |
| `roles_file` | string\|null | `null` | Section file holding roles |

The config file may be JSON or YAML (detected by extension, then content).
Derived defaults are applied at load ([`ProxyConfig::normalize`]): only `id` and
`caption` are required per dimension/measure; captions cascade to
`hierarchy_name`/`leaf_level_name`/`display_name`, `ordinal` follows list order,
`all_level_name` defaults to `(All)`, `visible` to true, `physical_field` to the
id, `format_string` to `#,##0.00`, and a measure's `measure_group_name` follows
its fact table (or the cube). `mallard fmt` writes the canonical form (defaults
omitted again) and converts formats; a rewrite does not preserve YAML comments.

When `fact_tables` is empty, the proxy uses single-fact mode with `source_name`/`table_name`.
When `fact_tables` is non-empty, all measures must declare `fact_table`.

### FactTableConfig

| Field | Type | Description |
|---|---|---|
| `id` | string | Unique identifier, referenced by dimension/measure `fact_table` fields |
| `source_name` | string | Legacy source name (informational) |
| `table_name` | string | DuckDB physical table name |
| `measure_group_name` | string | SSAS measure group name displayed to Excel |

### DimensionConfig

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | string | required | Internal identifier for QueryPlan/plan_key |
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
| `sql_expr` | string | required | DuckDB SQL expression (e.g. `"SUM(revenue)"`) — the actual runtime path |
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
When `null` or omitted, the proxy seeds a temporary DuckDB file with synthetic
data (20k rows of `sales_fact`) and serves that.

When set, the DuckDB backend opens the file directly. There is no separate
runtime — DuckDB is the only execution engine.

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

