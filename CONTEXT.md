# SSAS Proxy — Session Context

## Goal
Rust proxy that impersonates an SSAS server to satisfy Excel's MSOLAP client.
Eventually: transpile MDX → Malloy → DuckDB.
**Current status:** Excel PivotTable works correctly end-to-end (metadata,
probe, query, collapse/expand) on `Produktkategori`, `Region`, and
`Total Försäljning`. 81 unit tests. Full pipeline: `MDX -> ParsedMdx ->
SemanticQuery -> QueryPlan -> {Malloy, SQL}` dual emission. SemanticModel
drives both emitters. Runtime execution via generated SQL + generic backend
methods. Criterion benchmarks cover pipeline overhead + dataset-size scaling
(small=10k, medium=100k, large=1M rows). Key finding: Malloy/SQL emission
is negligible; execution engine is the bottleneck.

## Benchmark results (2026-05-24)

### Pipeline overhead (tiny demo dataset)
| Stage | Slicer | Drilldown | CrossJoin | Collapse |
|---|---|---|---|---|
| Parse | 2.3 µs | 4.4 µs | 4.4 µs | 5.9 µs |
| Plan | 37 ns | — | — | 14 ns |
| Emit SQL | 349 ns | 214 ns | 236 ns | — |
| Emit Malloy | 458 ns | 219 ns | 213 ns | — |
| Execute | 3.1 µs | — | — | — |
| E2E | 23 µs | 42 µs | 68 µs | 78 µs |

### Scale benchmarks (SQLite, execution-only)
| Query | 10k rows | 100k rows | 1M rows |
|---|---|---|---|
| Total | 250 µs | 2.5 ms | 26.5 ms |
| GroupBy 1D | 1.9 ms | 28.5 ms | — |
| GroupBy 1D filtered | 255 µs | 2.6 ms | — |
| GroupBy 2D | 3.1 ms | 44.5 ms | — |
| GroupBy 2D filtered | 257 µs | 2.6 ms | — |
| Collapse | 3.0 ms | 44.1 ms | — |

### Key findings
- **Malloy/SQL emission is effectively free** — sub-microsecond
- **Parse + plan is cheap** — 2-6 µs even for complex MDX
- **Execution dominates at 100k+ rows** — backend engine is the bottleneck
- **XMLA rendering adds ~10-20% overhead** — not the primary target
- **Filtered queries are much faster** due to selectivity
- **GroupBy queries degrade predictably with data size**

### Benchmark infrastructure
- `benches/pipeline.rs` — two Criterion groups: `pipeline` (overhead) + `scale` (sizes)
- `Backend::new_with_config()` — deterministic synthetic data generator
- `BenchmarkDataConfig::small/medium/large()` — preset profiles
- Backend-injected execution: `execute_plan_with_backend()`, `get_execute_cellset_response_with_backend()`
- Run with: `cargo bench --bench pipeline -- scale`

## Recent fixes

### Criterion benchmarks + scale harness
- Added `criterion` dev-dependency, `[[bench]]` target, `benches/pipeline.rs`.
- `src/lib.rs` — crate root restructured as lib+bin for benchmark access.
- `src/test_fixtures.rs` — shared MDX constants for tests and benches.
- `src/backend.rs`: `BenchmarkDataConfig`, `SeededRng`, `new_with_config()`,
  deterministic synthetic data generator with preset profiles.
- `engine/plan.rs`: `execute_plan_with_backend()` — injectable backend variant.
- `execute_builders.rs`: `execute_semantic_query_with_backend()`,
  `get_execute_cellset_response_with_backend()` — backend-injected e2e path.
- 6 query shapes benchmarked across 3 dataset sizes.

### ParsedMdx-driven classification
- `ParsedMdx` now carries query-shape flags: `CChildrenTarget`,
  `CalculatedMembersPat`, `has_drilldown_member`, `has_measures`.
- `semantic_query_from_mdx()` classifies using these structural fields
  instead of bare `contains(...)` chains.
- Old wrapper functions kept for test backward compat.

### Malloy-ready QueryPlan
- Renamed `ExecutionPlan` → `QueryPlan`. Simplified to 4 variants:
  `Total`, `GroupBy(dims, filters)`, `Count`, `Empty`.
- `QueryResult` mirrors: `Scalar`, `Grouped`, `Pairs`, `Count`, `Empty`.
- Collapse logic moved out of plan executor into collapse builder.

### Typed semantic IR
- Added `Dimension` enum (`Produktkategori`, `Region`) and `Measure` enum
  (`TotalSales`).
- `QueryPlan` uses typed `TypedDimensionFilter` instead of freeform strings.
- `plan_from_semantic()` converts string-based `DimensionFilter` to typed.

### Malloy emitter (engine/malloy.rs)
- `malloy_model(model)` and `malloy_query(model, plan)` emit separately.
- Supported shapes: Total, GroupBy(1 dim), GroupBy(2 dims), filters.
- Multiple filter members joined with `|`.

### SemanticModel (engine/model.rs)
- `SemanticModel`: typed source, dimensions, measures with physical field mappings.
- `DimensionDef`, `MeasureDef` with `semantic_name`, `physical_field/expr`, `sql_expr`.
- `default_model()`: static model for `faktatabell` dataset.

### SQL emitter (engine/sql.rs)
- `sql_for_query_plan(model, plan) -> String` generates SQL.
- Supports: Total, GroupBy(1), GroupBy(2), Count, filters with `WHERE … AND`.
- `MeasureDef` now has `sql_expr` (SQL) alongside `physical_expr` (Malloy).
- `backend.rs`: generic SQL query methods (`query_scalar`, `query_grouped_1d`,
  `query_pairs`, `query_count`).
- `execute_plan()` generates SQL from model + plan, no bespoke grouped query branching.

### `nom`-based MDX parser (mdx_parser.rs)
- Added `nom = "7"` dependency. Parses: member references, WHERE clauses,
  subquery filters, property clauses, axis shape detection.
- `mdx_semantic.rs` delegates property/filter/slicer parsing to `mdx_parser`.

### DrilldownMember expand/collapse support
- `SemanticQueryKind::DrilldownMemberProbe`, `excluded_members`, `drilldown_member_hierarchy`.
- `build_drilldown_member()` handles two collapse forms.

### Various early fixes
- CrossJoin SlicerAxis ordering, complete SlicerAxis, dimension-tagged filters,
  nested subquery filter merge, collapsed All member Axis0 property fix,
  PARENT_UNIQUE_NAME omission, debug logging.

## Project structure
```
xmla_proxy/
  Cargo.toml           — deps: axum, rusqlite, nom, criterion (dev)
  benches/
    pipeline.rs        — Criterion benchmarks (pipeline + scale groups)
  src/
    lib.rs             — Crate root (lib+bin), re-exports all public modules
    main.rs            — Thin binary entrypoint, uses lib modules
    test_fixtures.rs   — Shared MDX string constants (tests + benches)
    parser.rs          — parse_xmla() → XmlaRequest enum (quick-xml)
    response.rs        — SOAP envelope, rowset envelope, UUID_TYPE
    properties.rs      — DISCOVER_PROPERTIES (14-property registry)
    schema_rowsets.rs  — DISCOVER_SCHEMA_ROWSETS
    catalogs.rs        — DBSCHEMA_CATALOGS
    cubes.rs           — MDSCHEMA_CUBES
    tables.rs          — DBSCHEMA_TABLES
    dimensions.rs      — MDSCHEMA_DIMENSIONS
    hierarchies.rs     — MDSCHEMA_HIERARCHIES
    levels.rs          — MDSCHEMA_LEVELS
    measures.rs        — MDSCHEMA_MEASURES
    measure_groups.rs  — MDSCHEMA_MEASUREGROUPS
    measuregroup_dimensions.rs — MDSCHEMA_MEASUREGROUP_DIMENSIONS
    members.rs         — MDSCHEMA_MEMBERS
    mdschema_properties.rs — MDSCHEMA_PROPERTIES
    literals.rs        — DISCOVER_LITERALS
    sets.rs            — MDSCHEMA_SETS
    kpis.rs            — MDSCHEMA_KPIS
    tmschema.rs        — TMSCHEMA_* stubs
    execute.rs         — Thin dispatch + test module (81 tests)
    execute_builders.rs — Cellset builders + flat-rowset fallback
    axis_members.rs    — Member/cell/axis/slicer helpers
    mdx_semantic.rs    — Semantic model, classification via ParsedMdx
    mdx_parser.rs      — nom-based MDX parser
    backend.rs         — SQLite backend + benchmark data generator
    cellset.rs         — Cellset XML builder (mddataset)
    rowset.rs          — Rowset infrastructure (unused)
    engine/
      mod.rs           — Module declaration
      plan.rs          — QueryPlan, QueryResult, Dimension, Measure,
                         plan_from_semantic(), execute_plan(),
                         execute_plan_with_backend()
      model.rs         — SemanticModel, DimensionDef, MeasureDef,
                         default_model()
      malloy.rs        — Malloy emitter (model + query)
      sql.rs           — SQL emitter (model + query)
```

## What works
- Full discover handshake; Excel PivotTable works end-to-end.
- Single and two-dimension drilldown (CrossJoin).
- Cross-dimension filtering, slicer-only queries.
- Expand/collapse on 2-hierarchy axis (DrilldownMember).
- Probe queries: All.Members, All.Children, Leaf.Children, cChildren.
- `MDX -> ParsedMdx -> SemanticQuery -> QueryPlan -> {Malloy, SQL}` dual emission.
- Runtime execution via generated SQL + generic backend methods.
- Criterion benchmarks for pipeline + dataset-size scaling (10k-1M rows).
- Deterministic synthetic data generator with configurable profiles.
- 81 unit tests + 10 benchmark groups.

## What does not yet work
- Malloy is emitted but not compiled or executed at runtime.
- Full N-way MDX generalization.
- DuckDB backend (SQLite only).
- Some unused helper functions remain.
- Large (1M-row) GroupBy benchmarks time out — need DB engine comparison.

## Next workstreams (prioritised)

1. **Execution engine comparison.** **NEXT** — benchmark SQLite vs DuckDB
   on the same scale harness. GroupBy queries at 100k-1M rows are the target.
2. **DuckDB backend.** Swap execution engine for analytic queries.
3. **Runtime Malloy compilation.** Evaluate once backend choice is settled.
   Current data shows emission is free; compilation cost is the open question.
4. **File-structure reorg.** Group modules: `mdx/`, `engine/`, `builders/`, `metadata/`.
5. **Remove stale code.** Clean up unused helpers and warnings.

### Completed workstreams
1. **ExecutionPlan → QueryPlan.** **DONE**
2. **Query-kind from parsed MDX.** **DONE**
3. **Split execute_builders.rs.** **DONE**
4. **Malloy generation.** **DONE**
5. **SQL generation + generic execution.** **DONE**
6. **Criterion benchmarks + scale harness.** **DONE**

## Key lessons learned

13. **Excel uses `CrossJoin(DrilldownLevel(...), DrilldownLevel(...))` to
    place a second field on Rows.** Must build multi-hierarchy Axis0.

14. **SlicerAxis must contain every off-axis cube dimension** in stable order.

15. **Off-axis SlicerAxis members must use standard 5 properties only.**

16. **Classification order matters.** SlicerAllAndMeasure must gate behind
    drilldown/axis checks.

17. **Collapsed All members on visible axes need full dim props.**
    SlicerAxis-style properties cause "null value" rowset errors.

18. **`DrilldownMember(CrossJoin(...), excluded_set, hierarchy)` is the
    Excel 2-hierarchy collapse shape.**

19. **Malloy/SQL emission is effectively free** — sub-microsecond even for
    complex query shapes. The semantic pipeline is not the bottleneck.

20. **Backend execution dominates at scale.** GroupBy queries are the stress
    point. Engine choice (SQLite vs DuckDB) is the next important performance
    decision.

21. **XMLA rendering adds ~10-20% overhead** — not the primary optimization
    target. Backend engine swap will yield bigger gains.

## Hard-coded constants
- Catalog name: `KTH_KEX_MALLOY_CUBE`
- Cube name: `Model`
- Measure name: `Total Försäljning` (caption `Total Försäljning (SEK)`)
- Measure group: `Faktatabell`
- Dimensions: `Produktkategori`, `Region`
- Session ID: `RUST-SESSION-456` (in response.rs)
- Cube dimension ordinal order: `ALL_DIMS = ["Measures", "Produktkategori", "Region"]`
