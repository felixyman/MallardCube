# SSAS Proxy - Session Context

## Goal
- Keep `xmla_proxy/` fully Excel-compatible while advancing `MDX -> ParsedMdx -> SemanticQuery -> QueryPlan -> {Malloy, SQL}`.
- Long-term target: Malloy as semantic authority.
- Current safe runtime path: `QueryPlan -> SQL -> DuckDB`.

## Current status
- Excel metadata/discover handshake works across the required rowsets for PivotTable use.
- Excel PivotTable query execution works end-to-end for `Produktkategori`, `Region`, and `Total Forsaljning`.
- DuckDB is the default analytic backend.
- Full typed pipeline exists: `MDX -> ParsedMdx -> SemanticQuery -> QueryPlan -> {Malloy, SQL}`.
- `SemanticModel` is the shared authority for metadata rowsets, SQL emission, and Malloy emission.
- Malloy runtime feasibility is now proven through a long-lived Node worker path.
- Compile result now captures `compile_ms` from the JS worker and the runtime path actually executes the compiled SQL (not direct Rust SQL).
- Result-parity tests verify direct SQL and Malloy-compiled SQL produce identical results.
- Query-plan normalization and caches exist for Malloy source, SQL text, and compiled SQL.
- Test suite: 125 passing tests (worker-dependent tests run serially).

## Constraints and preferences
- Excel/MSOLAP compatibility is strict; rowset and cellset layout correctness matters.
- Prefer correctness over guessing.
- Keep probe/metadata/collapse quirks in Rust even if Malloy becomes the analytic runtime.
- Prefer minimal changes and shared model-driven behavior.
- Single-binary direction remains desirable, but practical spikes are acceptable.

## What we completed

### Excel metadata and XMLA compatibility
- Discover/metadata handshake works across:
  - `DISCOVER_PROPERTIES`
  - `DISCOVER_SCHEMA_ROWSETS`
  - `DBSCHEMA_CATALOGS`
  - `MDSCHEMA_CUBES`
  - `MDSCHEMA_DIMENSIONS`
  - `MDSCHEMA_HIERARCHIES`
  - `MDSCHEMA_LEVELS`
  - `MDSCHEMA_MEASURES`
  - `MDSCHEMA_PROPERTIES`
  - `MDSCHEMA_MEMBERS`
- `MDSCHEMA_MEMBERS` was aligned to the spec:
  - corrected `TREE_OP`
  - corrected `MEMBER_TYPE`
  - corrected column ordering
  - added missing fields such as `PARENT_UNIQUE_NAME`, `MEMBER_KEY`, `MEMBER_GUID`
  - parent/ancestor handling now uses real parent names

### Cellset and axis behavior
- `src/cellset.rs` now supports:
  - multi-member tuples
  - multiple hierarchies per axis
  - conditional `CellInfo` and `CellData` property emission
- `SlicerAxis` behavior fixed:
  - includes every off-axis dimension
  - stable ordering by metadata ordinal
  - default `All` members emitted
  - off-axis members use the standard 5-property shape

### Semantic and parser layers
- `ParsedMdx`-driven classification replaced brittle string matching.
- Added shape/classification flags including:
  - `CChildrenTarget`
  - `CalculatedMembersPat`
  - `has_drilldown_member`
  - `has_measures`
- Typed semantic IR implemented:
  - `Dimension`
  - `Measure`
  - `TypedDimensionFilter`
  - `QueryPlan`
  - `QueryResult`

### Model-driven architecture
- `SemanticModel` implemented with:
  - `DimensionDef`
  - `MeasureDef`
  - typed metadata for captions, descriptions, ordinals, levels, formatting, GUIDs, and unique-name helpers
- These rowsets now generate from `default_model()`:
  - `measure_groups.rs`
  - `measuregroup_dimensions.rs`
  - `mdschema_properties.rs`
  - `dimensions.rs`
  - `hierarchies.rs`
  - `levels.rs`
  - `measures.rs`
- `Measures` and `MeasuresLevel` still keep necessary special-casing.

### Query shapes and filtering
- Added `Region` as a second visible dimension across metadata and execution.
- Filter extraction works for:
  - `WHERE ([Produktkategori]...&[Kategori X], [Measures]...)`
  - `WHERE ([Region]...&[North], [Measures]...)`
  - subquery `SELECT ({...})`
  - nested subqueries merged by dimension
- Cross-dimension row/filter flows work in both directions.

### CrossJoin and collapse support
- Added CrossJoin support via:
  - `SemanticQuery.axis_dimensions: Vec<String>`
  - `build_drilldown_multi()`
  - 2D grouped execution
- `DrilldownMember(...)` collapse support implemented and fixed:
  - `SemanticQueryKind::DrilldownMemberProbe`
  - `drilldown_member_hierarchy`
  - `ExcludedMember { dimension, key }`
  - symmetric collapse regardless of row order
  - axis-order-aware tuple rendering
  - dimension-tagged excluded-member parsing
  - 2D SQL row interpretation fixed using actual axis order

### Backend and emitters
- DuckDB replaced SQLite as the default backend in `src/backend.rs`.
- Generic SQL query methods and `QueryBackend` trait were preserved.
- Synthetic benchmark/test data generation was preserved.
- SQL emitter implemented in `engine/sql.rs`.
- Malloy emitter implemented in `engine/malloy.rs`.
- Malloy model emission no longer redundantly redefines dimensions when `semantic_name == physical_field`.
- Runtime execution currently uses generated SQL by default.

### Caching and normalization
- `engine/normalize.rs` provides `plan_key(plan)`.
- `engine/cache.rs` contains:
  - SQL cache
  - Malloy source cache
  - compiled SQL cache
  - hit/miss counters

### Malloy compiler/runtime work
- `engine/malloy_compiler.rs` defines:
  - `CompileResult { sql, compile_ms }` — carries JS-side compile timing
  - `MalloyCompiler` trait returning `Result<CompileResult, MalloyCompileError>`
  - `NullCompiler`
- `engine/malloy_node.rs` provides a one-shot Node compiler spike.
- `engine/malloy_node_longlived.rs` provides a long-lived Node worker client that captures `compile_ms` from the JS response.
- `execute_plan_with_sql()` in `plan.rs` executes generic SQL against the backend (used by the Malloy runtime path).
- JS tooling added:
  - `package.json`
  - `js/malloy-cli.js`
  - `js/malloy-worker.js` — returns `compile_ms` in responses
  - `js/malloy_rquickjs_entry.js`
  - `build/malloy-compiler.bundle.js`
- Critical runtime finding: Malloy compilation requires a real DuckDB schema/connection at compile time.
- Proven runtime behavior:
  - one-shot compile works but is too slow at about 550 ms/request
  - long-lived warm compile works
  - compiled-query cache hits are effectively free
  - currently supported subset includes `Total`, `GroupBy(1)`, `GroupBy(2)`, and filtered analytic queries
  - `Count` and `Empty` are still rejected on the Malloy path
  - result parity confirmed: Malloy-compiled SQL produces same results as direct Rust SQL (4 parity tests)

### Runtime instrumentation
- `MALLOY_RUNTIME=1` enables the Malloy runtime path in `src/main.rs`.
- Timed execution/logging path exists and emits `TIMINGS ...` lines.
- Runtime path labels include:
  - `DirectSql`
  - `MalloyCompiled`
  - `MalloyCached`
- Real logs already show repeated `plan_key`s and cache hits.

### Benchmarks and docs
- Criterion benchmark infrastructure added:
  - `benches/pipeline.rs`
  - `src/lib.rs`
  - `src/test_fixtures.rs`
- Architecture docs added under `docs/`:
  - `current-architecture.mmd`
  - `target-architecture.mmd`
  - `migration-plan.mmd`
  - `collapse-sequence.mmd`
  - `README.md`

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
- One-shot Malloy compile is not interactive-use viable, but long-lived compile is sub-ms warm.
- Cache hit rate is likely the deciding factor for whether runtime Malloy is practical.
- Real Excel logs already show repeated `plan_key`s, which keeps Malloy runtime promising.
- XMLA rendering overhead is not the main scaling problem.
- Direct Rust SQL and Malloy-compiled SQL produce identical results for the supported analytic subset.
- Malloy internal caching makes "cold" vs "warm" distinction subtle; unique-query comments don't fully defeat it.

## Current gaps and risks
- `src/members.rs` is still the most manual and sensitive metadata path.
- Timing instrumentation still has known correctness bugs:
  - `semantic_us` is often `0`
  - `sql_emit_us` is often `0`
  - `malloy_emit_us` is often `0`
  - `total_us` can be smaller than component timings
- Long-lived worker usage is not yet hardened for robust concurrent use.
- Worker-related tests currently require serialization: `cargo test --lib -- --test-threads=1`.
- Embedded `rquickjs` path has not yet been meaningfully proven.
- Malloy's internal caching means "cold" unique-source benchmarks may not reflect true fresh compile cost.

## What works today
- Full discover handshake for Excel/MSOLAP.
- End-to-end PivotTable execution for current cube shape.
- Multi-hierarchy Rows axis via CrossJoin.
- Cross-dimension filtering and nested filter merge.
- Symmetric collapse/expand behavior regardless of row order.
- Typed `ParsedMdx -> SemanticQuery -> QueryPlan` flow.
- SQL generation and DuckDB execution.
- Malloy emission and long-lived compilation spike.
- Malloy-compiled SQL execution via `execute_plan_with_sql`.
- Result parity between direct Rust SQL and Malloy-compiled SQL (4 tests).
- JS-side `compile_ms` captured from worker into `CompileResult`.
- Cache normalization and cache hit logging.
- 125 passing tests in the current suite.

## Current priorities
1. Continue moving remaining manual metadata logic onto stronger model abstractions, especially `src/members.rs`.
2. Fix remaining timing instrumentation correctness (`semantic_us`, `sql_emit_us`, `malloy_emit_us`, `total_us` boundaries).
3. Measure real Excel cache-hit behavior from `debug-last-run.log`.
4. Compare direct SQL vs Malloy runtime on actual Excel interactions (using the now-correct Malloy execute path).
5. Harden the long-lived worker if Malloy runtime continues to look viable.
6. Evaluate Malloy cold-compile cost by measuring full `compile_ms` from the worker (already captured in `CompileResult`).

## Relevant files
- `src/backend.rs`: DuckDB default backend, `QueryBackend`, synthetic data generation.
- `src/execute.rs`: thin dispatch and regression tests.
- `src/execute_builders.rs`: execution dispatch, Malloy runtime toggle, timed paths, collapse/axis handling.
- `src/mdx_semantic.rs`: semantic parsing, typed filters, excluded members, axis-order behavior.
- `src/mdx_parser.rs`: `nom` parser and `ParsedMdx`.
- `src/axis_members.rs`: member/cell/axis/slicer helpers.
- `src/cellset.rs`: XMLA cellset builder.
- `src/engine/plan.rs`: `QueryPlan`, `QueryResult`, `execute_plan_with_sql` (for Malloy-compiled SQL), direct SQL execution path.
- `src/engine/model.rs`: authoritative `SemanticModel` and metadata definitions.
- `src/engine/malloy.rs`: Malloy emission.
- `src/engine/sql.rs`: SQL emission.
- `src/engine/normalize.rs`: `plan_key(plan)`.
- `src/engine/cache.rs`: source/SQL/compiled cache layers.
- `src/engine/malloy_compiler.rs`: `CompileResult`, `MalloyCompiler` trait, compile errors.
- `src/engine/malloy_node.rs`: one-shot Node compile spike.
- `src/engine/malloy_node_longlived.rs`: long-lived worker client captures `compile_ms` from JS worker.
- `src/engine/parity.rs`: direct-SQL vs Malloy-path parity coverage.
- `src/engine/timing.rs`: `Timings` and runtime path data.
- `src/members.rs`: remaining manual/high-risk metadata implementation.
- `src/main.rs`: runtime toggle and request timing/logging.
- `benches/pipeline.rs`: pipeline and scaling benchmarks.
- `debug-last-run.log`: latest Excel request/response/timing trace.
- `docs/`: architecture and migration diagrams.

## Constants and current cube shape
- Catalog name: `KTH_KEX_MALLOY_CUBE`
- Cube name: `Model`
- Measure: `Total Forsaljning` with caption `Total Forsaljning (SEK)`
- Measure group: `Faktatabell`
- Dimensions: `Produktkategori`, `Region`
- Session ID: `RUST-SESSION-456`
