# SSAS Proxy - Session Context

## Goal
- Keep `xmla_proxy/` fully Excel-compatible while advancing `MDX -> ParsedMdx -> SemanticQuery -> QueryPlan -> {Malloy, SQL}`.
- Long-term target: Malloy as semantic authority; proxy owns only the Excel/XMLA compatibility layer.
- Current safe runtime path: `QueryPlan -> SQL -> DuckDB`, with Malloy runtime path behind `MALLOY_RUNTIME=1`.

## Current status
- Excel metadata/discover handshake works across required rowsets for PivotTable use.
- Excel PivotTable query execution works end-to-end for current cube shape.
- DuckDB is the default analytic backend.
- Full typed pipeline: `MDX -> ParsedMdx -> SemanticQuery -> QueryPlan -> {Malloy, SQL}`.
- **Core abstraction is model-driven, not demo-cube-specific:**
  - `DimId` / `MeasId` are `String` type aliases — no hardcoded enums.
  - Planning uses `default_measure_id()` / `default_dimension_id()` from the loaded model.
  - MDX dimension detection scans configured dimensions, not hardcoded candidates.
  - `members.rs` queries DuckDB for distinct values — no hardcoded business members.
  - Collapse/2D rendering is generic — uses `axis_dimensions` order, not fixed dimension names.
- **Three independent sample projects prove the abstraction:**
  - `project/` — original demo (Produktkategori, Region, TotalSales).
  - `project2/` — renamed (Category, Territory, Revenue) against same physical data.
  - `project3/` — wider model (Category, Territory, Channel, Segment, Revenue, Units) against `sales_fact`.
- **Proxy config + Malloy model file loading works at startup** (`PROXY_CONFIG=...`).
  - Developer-supplied `.malloy` file is loaded as the Malloy model source.
  - `proxy-config.json` maps Malloy names to Excel/XMLA captions, formatting, ordering.
  - Without config, defaults to `project3/proxy-config.json`.
- Malloy runtime path (long-lived Node worker) compiles and executes developer-owned Malloy.
- Compile result carries `compile_ms` from the JS worker; runtime path executes compiled SQL.
- Result-parity tests verify direct SQL and Malloy-compiled SQL produce identical results.
- Worker spawn + warm-up happens eagerly at server startup.
- Query-plan normalization and caches exist for Malloy source, SQL text, compiled SQL.
- Naming contract documented in `docs/naming-contract.md`.
- File reorg complete — architecture boundaries now reflected in directory layout.
- Test suite: **152 passing tests** (worker-dependent tests run serially).
- **Shared DuckDB runtime**: `db_path` in config controls file-based (real) vs in-memory (demo). Rust backend and JS worker share same DB file.
- Seed data tooling: `cargo run --bin seed_sql > seed.sql`, `data/` with `.db`, `.parquet`, `.sql`.
- SSAS Tabular conversion reference: `docs/ssas-to-malloy-conversion.md` — system prompt for `.bim` → Malloy + DuckDB.

## Supported simple-model scope (explicit boundary)
- One DuckDB fact source.
- Flat dimensions from columns.
- One hierarchy per dimension with `(All)` + one leaf level.
- Aggregate measures from Malloy.
- Up to 2 visible row dimensions for current Excel Pivot interactions.
- Not yet: multi-fact-table, arbitrary N-way hierarchies, multi-source joins, Postgres/MSSQL ingestion.

## Constraints and preferences
- Excel/MSOLAP compatibility is strict; rowset and cellset layout correctness matters.
- Prefer correctness over guessing.
- Keep probe/metadata/collapse quirks in Rust even if Malloy becomes the analytic runtime.
- Prefer minimal changes and shared model-driven behavior.
- Single-binary direction remains desirable, but practical spikes are acceptable.

## What we completed

### Excel metadata and XMLA compatibility
- Discover/metadata handshake works across:
  `DISCOVER_PROPERTIES`, `DISCOVER_SCHEMA_ROWSETS`, `DBSCHEMA_CATALOGS`,
  `MDSCHEMA_CUBES`, `MDSCHEMA_DIMENSIONS`, `MDSCHEMA_HIERARCHIES`, `MDSCHEMA_LEVELS`,
  `MDSCHEMA_MEASURES`, `MDSCHEMA_PROPERTIES`, `MDSCHEMA_MEMBERS`.
- `MDSCHEMA_MEMBERS` aligned to spec and queries DuckDB for actual member values.

### Cellset and axis behavior
- Multi-member tuples, multiple hierarchies per axis, conditional `CellInfo`/`CellData`.
- `SlicerAxis`: every off-axis dimension, stable ordering, default `All`, standard 5-property shape.

### Semantic and parser layers
- `ParsedMdx`-driven classification with shape flags.
- Typed semantic IR: `DimensionFilter`, `ExcludedMember`, `SemanticQuery`, `SemanticQueryKind`.

### Dynamic semantic model
- **`DimId = String`, `MeasId = String`** — no compile-time enums.
- `SemanticModel`, `DimensionDef`, `MeasureDef` use owned types, loadable from config.
- Metadata rowsets (`dimensions`, `hierarchies`, `levels`, `measures`, etc.) generate from model.
- Runtime helpers: `default_measure_id()`, `default_dimension_id()`, `lookup_dimension()`.

### Proxy config and project loading
- `src/proxy_config.rs`: JSON config schema (`ProxyConfig`, `DimensionConfig`, `MeasureConfig`).
- `src/proxy_project.rs`: Loads `.malloy` file + config, builds `SemanticModel`, provides `malloy_source()`.
- `project/`, `project2/`, `project3/`: three independent sample projects proofing the abstraction.
- `db_path` field in config enables real DuckDB file mode (demo mode when null).

### Generic execution layer
- `plan_from_semantic()` uses model-derived defaults — no hardcoded measure/dimension names.
- MDX dimension detection scans configured dimensions via `lookup_dimension()`.
- `members.rs` queries DuckDB `DISTINCT` values, uses project config for catalog/cube names.
- 1D/2D tuple rendering in `execute_builders.rs` uses `axis_dimensions` order.
- Collapse logic is dimension-independent — handles exclusions on either dimension.

### Query shapes and filtering
- `Region`/`Territory` as second dimension. Cross-dimension row/filter flows.
- Filter extraction: WHERE, subquery SELECT, nested subqueries merged by dimension.

### CrossJoin and collapse
- Multi-hierarchy Rows via `CrossJoin`, 2D grouped execution.
- `DrilldownMember(...)` collapse: symmetric collapse regardless of row order.
- Generic collapsed-total computation from result data (no Backend-specific aggregate calls).

### Backend and emitters
- DuckDB backend with `distinct_count()` and `distinct_values()` helpers.
- SQL emitter and Malloy emitter with loaded-model-text support (`malloy_source_with_model_text()`).
- `execute_plan_with_sql()` for Malloy-compiled SQL execution.
- `Backend::open(path)` for file-based DuckDB. `init_backend(Option<&str>)` for config-driven init.

### Caching and normalization
- `plan_key(plan)` normalization. SQL, Malloy, and compiled-SQL caches with hit/miss counters.

### Malloy compiler/runtime work
- `CompileResult { sql, compile_ms }` — JS-side compile timing from worker.
- `MalloyCompiler` trait, `NullCompiler`, one-shot Node spike, long-lived worker.
- Result parity confirmed (4 tests). Worker warm-up at startup.
- `malloy_compile_warm`, `malloy_compile_cold`, `malloy_compile_cached` benchmarks.
- Malloy compile errors no longer silently swallowed — logged to stderr with plan key, kind, measure, and full generated source; automatic fallback to direct SQL.
- `js/proxy-schema.js` derives compile-time DuckDB schema from Malloy source; WHERE clause extraction uses word-boundary regex to capture all filter columns.
- Shared DuckDB runtime: `db_path` in config controls file-based (real) vs in-memory (demo). Both Rust and JS worker open the same DB file. Malloy compiles against real schema.
- Worker passes `DUCKDB_PATH` env var from config's resolved `db_path`. Conditional connection in `malloy-worker.js`.

### Runtime instrumentation
- `MALLOY_RUNTIME=1` toggle. Timed execution path with runtime path labels.
- `js_compile_ms` field in `Timings` and log output.

### Data tooling
- `src/bin/seed_sql.rs`: generates `sales_fact` seed SQL from `generate_sales_fact_rows()`.
- `data/seed.sql`, `data/sales.db`, `data/sales_fact.parquet`: ready-to-use synthetic data.

### Documentation
- `docs/naming-contract.md` — id/caption/malloy_name naming rules.
- `docs/DIAGRAMS.md` — architecture diagrams (moved from old `docs/README.md`).
- `docs/cellset-reference.md` — XMLA cellset layout reference.
- `docs/ssas-to-malloy-conversion.md` — comprehensive `.bim` → Malloy + DuckDB conversion reference (10 sections, 3 appendices, designed as LLM system prompt).
- `README.md` at repo root — quick start, project structure, connecting Excel, demo vs real data, tests.

## Benchmark results

### DuckDB scale benchmarks
| Query | 10k rows | 100k rows | 1M rows |
|---|---|---|---|
| Total | 209 us | 485 us | 868 us |
| GroupBy 1D | ~2 ms | 2.78 ms | - |
| GroupBy 2D | ~3 ms | 4.67 ms | - |
| Collapse | ~3 ms | 4.60 ms | - |

### DuckDB vs SQLite at 100k rows
| Query | SQLite | DuckDB | Speedup |
|---|---|---|---|
| Total | 2.4 ms | 485 us | 5x |
| GroupBy 1D | 27.5 ms | 2.78 ms | 10x |
| GroupBy 2D | 42.6 ms | 4.67 ms | 9x |
| Collapse | 42.7 ms | 4.60 ms | 9x |

### Malloy runtime benchmarks
- Long-lived warm compile `total`: about 886-914 us
- Long-lived warm compile `group2d`: about 652-669 us
- Cached compile hit: about 300 ns

## Key findings
- DuckDB is 5-10x faster than SQLite for grouped analytic queries.
- `QueryPlan -> SQL` emission is extremely cheap; execution dominates real latency.
- One-shot Malloy compile not interactive-use viable, but long-lived compile is sub-ms warm.
- Direct Rust SQL and Malloy-compiled SQL produce identical results (parity tests).
- Malloy internal caching makes "cold" vs "warm" distinction subtle.
- Three independent projects with different naming prove the model-driven abstraction is real.

## Current gaps and risks
- **Multi-fact-table support**: `SemanticModel` has one `source_name`/`table_name` — measures and dimensions are all scoped to a single flat table. Phased plan designed (A: `FactTable` struct, B: emitters, C: config, D: metadata, E: star schema, F: multi-query merge).
- `execute_builders.rs` is the main complexity hotspot — collapse/tuple logic needs extraction.
- Some fallback defaults in planning (`default_measure_id()` etc.) can mask misconfiguration.
- Only "simple model" shape is supported: one fact source, flat dimensions, one hierarchy each.
- Timing instrumentation still has known correctness bugs in some fields.
- Long-lived worker not hardened for concurrent use; tests require serialization.
- File structure organised by architectural layer (`backend/`, `project/`, `mdx/`, `execute/`, `xmla/`, `engine/`, `test_support/`).
- `mdx_semantic.rs` still has some string-heuristic fragility for more complex MDX.

## What works today
- Full discover handshake for Excel/MSOLAP.
- End-to-end PivotTable execution for current cube shape.
- Multi-hierarchy Rows axis via CrossJoin. Cross-dimension filtering. Symmetric collapse.
- Typed `ParsedMdx -> SemanticQuery -> QueryPlan` flow with model-driven IDs.
- SQL generation and DuckDB execution. Malloy emission + compilation via long-lived worker.
- `execute_plan_with_sql()` for Malloy-compiled SQL execution.
- Result parity between direct SQL and Malloy-compiled SQL (4 tests).
- JS-side `compile_ms` captured. Eager worker warm-up at startup.
- Cache normalization and logging.
- Three independent sample projects load and work against different physical data.
- Shared DuckDB runtime with file and demo modes.
- Malloy compile fallback to direct SQL on failure.
- Seed data generation tooling (SQL, DB, Parquet).
- 152 passing tests.

## Current priorities
1. **Multi-fact-table support** — Phase A (model layer, zero behavior change, backward compat).
2. Extract collapse/tuple helpers from `execute_builders.rs`.
3. Harden the long-lived worker for concurrent use.
4. Fix remaining timing instrumentation correctness.
5. Time intelligence long-term plan (date_dim auto-generation, proxy config `time_intelligence` blocks, Excel MDX time functions).
6. File reorg after multi-fact-table boundaries settle.

## Relevant files
- `src/backend/mod.rs`: DuckDB default backend, `QueryBackend`, `distinct_count`, `distinct_values`, `Backend::open()`, `init_backend()`.
- `src/execute/dispatch.rs`: thin dispatch and regression tests (152 tests).
- `src/execute/builders.rs`: execution dispatch, Malloy runtime toggle, timed paths, generic 1D/2D/collapse rendering, Malloy compile fallback.
- `src/execute/axis_members.rs`: member/cell/axis/slicer helpers.
- `src/mdx/semantic.rs`: model-driven dimension detection, semantic classification, excluded-member parsing.
- `src/mdx/parser.rs`: `nom` parser and `ParsedMdx`.
- `src/xmla/cellset.rs`: XMLA cellset builder.
- `src/xmla/parser.rs`: XMLA envelope parser.
- `src/xmla/response.rs`: XMLA response helpers.
- `src/xmla/rowset.rs`: rowset builder.
- `src/xmla/discover/`: discover rowset implementations (catalogs, cubes, dimensions, measures, hierarchies, levels, members, sets, kpis, tmschema, etc.).
- `src/engine/plan.rs`: `DimId`, `MeasId`, `QueryPlan`, `QueryResult`, `plan_from_semantic` (model-driven), `execute_plan_with_sql`.
- `src/engine/model.rs`: `SemanticModel`, `DimensionDef`, `MeasureDef` with helpers (`default_measure_id`, `lookup_dimension`, etc.).
- `src/engine/malloy.rs`: Malloy emitter with `malloy_source_with_model_text()`.
- `src/engine/sql.rs`: SQL emitter.
- `src/engine/normalize.rs`: `plan_key(plan)`.
- `src/engine/cache.rs`: source/SQL/compiled cache layers.
- `src/engine/malloy_compiler.rs`: `CompileResult`, `MalloyCompiler` trait.
- `src/engine/malloy_node.rs`: one-shot Node compile spike.
- `src/engine/malloy_node_longlived.rs`: long-lived worker client captures `compile_ms`, passes `DUCKDB_PATH` env var.
- `src/engine/parity.rs`: direct-SQL vs Malloy-path result parity.
- `src/engine/timing.rs`: `Timings` with `js_compile_ms`.
- `src/project/config.rs`: JSON config schema, `db_path` field.
- `src/project/project.rs`: Project loader — builds `SemanticModel` from config, provides `malloy_source()`.
- `src/main.rs`: runtime toggle, project init, `Backend::init()` startup.
- `src/bin/seed_sql.rs`: generates `sales_fact` seed SQL from existing generation code.
- `src/test_support/fixtures.rs`: shared MDX test fixtures.
- `project3/model.malloy`, `project3/proxy-config.json`: wider sample project (4 dims, 2 measures). Default startup.
- `project2/model.malloy`, `project2/proxy-config.json`: renamed sample project.
- `project/model.malloy`, `project/proxy-config.json`: original demo project.
- `data/seed.sql`, `data/sales.db`, `data/sales_fact.parquet`: synthetic test data.
- `js/malloy-worker.js`: long-lived JS worker, conditional DuckDB connection (file vs :memory:).
- `js/proxy-schema.js`: compile-time DuckDB schema extraction from Malloy source.
- `js/malloy-cli.js`: one-shot Malloy compiler.
- `docs/DIAGRAMS.md`: architecture diagrams index.
- `docs/naming-contract.md`: id/caption/malloy_name naming rules.
- `docs/cellset-reference.md`: XMLA cellset layout reference.
- `docs/ssas-to-malloy-conversion.md`: `.bim` → Malloy + DuckDB conversion reference.
- `README.md`: project README with quick start, config walkthrough, Excel connection, architecture links.
- `benches/pipeline.rs`: pipeline, scale, and Malloy runtime benchmarks.
- `debug-last-run.log`: latest Excel request/response/timing trace.
- `benches/pipeline.rs`: pipeline, scale, and Malloy runtime benchmarks.
