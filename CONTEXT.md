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
- **Core abstraction is now model-driven, not demo-cube-specific:**
  - `DimId` / `MeasId` are `String` type aliases — no hardcoded enums.
  - Planning uses `default_measure_id()` / `default_dimension_id()` from the loaded model.
  - MDX dimension detection scans configured dimensions, not hardcoded candidates.
  - `members.rs` queries DuckDB for distinct values — no hardcoded business members.
  - Collapse/2D rendering is generic — uses `axis_dimensions` order, not fixed dimension names.
- **Two independent sample projects prove the abstraction:**
  - `project/` — original demo (Produktkategori, Region, TotalSales).
  - `project2/` — renamed (Category, Territory, Revenue) against same physical data.
  - Both load and work without code changes.
- **Proxy config + Malloy model file loading works at startup** (`PROXY_CONFIG=...`).
  - Developer-supplied `.malloy` file is loaded as the Malloy model source.
  - `proxy-config.json` maps Malloy names to Excel/XMLA captions, formatting, ordering.
  - Without config, `default_model()` fallback (backward-compatible).
- Malloy runtime path (long-lived Node worker) compiles and executes developer-owned Malloy.
- Compile result carries `compile_ms` from the JS worker; runtime path executes compiled SQL.
- Result-parity tests verify direct SQL and Malloy-compiled SQL produce identical results.
- Worker spawn + warm-up happens eagerly at server startup.
- Query-plan normalization and caches exist for Malloy source, SQL text, compiled SQL.
- Naming contract documented in `docs/naming-contract.md`.
- File reorg deferred — do after architecture boundaries fully settle.
- Test suite: **136 passing tests** (worker-dependent tests run serially).

## Supported simple-model scope (explicit boundary)
- One DuckDB fact source.
- Flat dimensions from columns.
- One hierarchy per dimension with `(All)` + one leaf level.
- Aggregate measures from Malloy.
- Up to 2 visible row dimensions for current Excel Pivot interactions.
- Not yet: arbitrary N-way hierarchies, multi-source joins, Postgres/MSSQL ingestion.

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
- `MDSCHEMA_MEMBERS` aligned to spec and now queries DuckDB for actual member values.

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
- `project/` and `project2/`: two independent sample projects. Startup via `PROXY_CONFIG=...` env var.

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

### Caching and normalization
- `plan_key(plan)` normalization. SQL, Malloy, and compiled-SQL caches with hit/miss counters.

### Malloy compiler/runtime work
- `CompileResult { sql, compile_ms }` — JS-side compile timing from worker.
- `MalloyCompiler` trait, `NullCompiler`, one-shot Node spike, long-lived worker.
- Result parity confirmed (4 tests). Worker warm-up at startup.
- `malloy_compile_warm`, `malloy_compile_cold`, `malloy_compile_cached` benchmarks.

### Runtime instrumentation
- `MALLOY_RUNTIME=1` toggle. Timed execution path with runtime path labels.
- `js_compile_ms` field in `Timings` and log output.

### Benchmarks and docs
- Criterion benchmarks for pipeline overhead, DuckDB scaling, Malloy runtime.
- `docs/naming-contract.md` — id/caption/malloy_name naming rules.
- Architecture diagrams in `docs/`.

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
- Two independent projects with different naming prove the model-driven abstraction is real.

## Current gaps and risks
- `execute_builders.rs` is the main complexity hotspot — collapse/tuple logic needs extraction.
- Some fallback defaults in planning (`default_measure_id()` etc.) can mask misconfiguration.
- Only "simple model" shape is supported: one fact source, flat dimensions, one hierarchy each.
- Timing instrumentation still has known correctness bugs in some fields.
- Long-lived worker not hardened for concurrent use; tests require serialization.
- File structure is still flat — reorg deferred until architecture boundaries settle.
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
- Two independent sample projects load and work against same physical data.
- 136 passing tests.

## Current priorities
1. File reorg after architecture boundaries fully settle (deferred from this pass).
2. Harden the long-lived worker for concurrent use.
3. Fix remaining timing instrumentation correctness.
4. Add proof case where `caption != id` (stretch the abstraction without breaking it).
5. Extract collapse/tuple helpers from `execute_builders.rs`.
6. Generalize beyond simple-model scope if needed (not yet urgent).

## Relevant files
- `src/backend.rs`: DuckDB default backend, `QueryBackend`, `distinct_count`, `distinct_values`.
- `src/execute.rs`: thin dispatch and regression tests (136 tests).
- `src/execute_builders.rs`: execution dispatch, Malloy runtime toggle, timed paths, generic 1D/2D/collapse rendering.
- `src/mdx_semantic.rs`: model-driven dimension detection, semantic classification, excluded-member parsing.
- `src/mdx_parser.rs`: `nom` parser and `ParsedMdx`.
- `src/axis_members.rs`: member/cell/axis/slicer helpers.
- `src/cellset.rs`: XMLA cellset builder.
- `src/members.rs`: queries DuckDB for member values, uses project config for catalog/cube.
- `src/engine/plan.rs`: `DimId`, `MeasId`, `QueryPlan`, `QueryResult`, `plan_from_semantic` (model-driven), `execute_plan_with_sql`.
- `src/engine/model.rs`: `SemanticModel`, `DimensionDef`, `MeasureDef` with helpers (`default_measure_id`, `lookup_dimension`, etc.).
- `src/engine/malloy.rs`: Malloy emitter with `malloy_source_with_model_text()`.
- `src/engine/sql.rs`: SQL emitter.
- `src/engine/normalize.rs`: `plan_key(plan)`.
- `src/engine/cache.rs`: source/SQL/compiled cache layers.
- `src/engine/malloy_compiler.rs`: `CompileResult`, `MalloyCompiler` trait.
- `src/engine/malloy_node.rs`: one-shot Node compile spike.
- `src/engine/malloy_node_longlived.rs`: long-lived worker client captures `compile_ms`.
- `src/engine/parity.rs`: direct-SQL vs Malloy-path result parity.
- `src/engine/timing.rs`: `Timings` with `js_compile_ms`.
- `src/proxy_config.rs`: JSON config schema.
- `src/proxy_project.rs`: Project loader — builds `SemanticModel` from config, provides `malloy_source()`.
- `src/main.rs`: runtime toggle, project init (`PROXY_CONFIG` env var), request timing/logging.
- `project/model.malloy`, `project/proxy-config.json`: sample project 1.
- `project2/model.malloy`, `project2/proxy-config.json`: sample project 2 (different names).
- `docs/naming-contract.md`: id/caption/malloy_name naming rules.
- `docs/`: architecture and migration diagrams.
- `benches/pipeline.rs`: pipeline, scale, and Malloy runtime benchmarks.
- `debug-last-run.log`: latest Excel request/response/timing trace.
