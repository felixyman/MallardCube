# SSAS Proxy — Session Context

## Goal
Rust proxy that impersonates an SSAS server to satisfy Excel's MSOLAP client.
Long term: transpile MDX → Malloy → DuckDB.
**Current status:** Excel PivotTable works correctly end-to-end on
`Produktkategori`, `Region`, and `Total Försäljning`. DuckDB is the
default backend. Full pipeline: `MDX -> ParsedMdx -> SemanticQuery ->
QueryPlan -> {Malloy, SQL}` dual emission. SemanticModel drives both
emitters. Runtime execution via generated SQL. Criterion benchmarks
cover pipeline overhead + scaling to 1M rows. 90 unit tests. Symmetric
2-hierarchy collapse works regardless of row order.

## Benchmark results (2026-05-24)

### DuckDB scale benchmarks (Malloy-compatible analytic queries only / execution)

| Query | 10k rows | 100k rows | 1M rows |
|---|---|---|---|
| Total | 209 µs | 485 µs | 868 µs |
| GroupBy 1D | ~2 ms | 2.69 ms | — |
| GroupBy 2D | ~3 ms | 4.63 ms | — |
| Collapse | ~3 ms | 4.82 ms | — |

### DuckDB vs SQLite (100k rows)

| Query | SQLite | DuckDB | Speedup |
|---|---|---|---|
| Total | 2.4 ms | 485 µs | **5x** |
| GroupBy 1D | 27.5 ms | 2.78 ms | **10x** |
| GroupBy 2D | 42.6 ms | 4.67 ms | **9x** |
| Collapse | 42.7 ms | 4.60 ms | **9x** |

### Pipeline overhead (tiny demo dataset)
| Stage | Parse | Plan | Emit SQL | Emit Malloy | Execute | E2E |
|---|---|---|---|---|---|---|
| | 2-6 µs | 14-37 ns | 200-350 ns | 210-460 ns | ~3 µs | 23-78 µs |

## Key findings
- **DuckDB is 5-10x faster than SQLite** for grouped analytic queries
- **Malloy/SQL emission is effectively free** — sub-microsecond
- **Execution dominates at 100k+ rows** — engine choice is the primary scaling factor
- **XMLA rendering adds ~10-20% overhead** — not the primary optimization target
- **Filtered queries are much faster** due to selectivity

## Recent fixes

### DuckDB as default backend
- Replaced SQLite with DuckDB backend. `Backend` now uses `duckdb::Connection`.
- Removed `backend_duckdb.rs` — `Backend` IS DuckDB now.
- `QueryBackend` trait: generic SQL execution interface.
- `FactRow` + `generate_rows()`: shared synthetic data generator for both backends.
- Benchmarks compare DuckDB vs SQLite on same harness (5-10x speedup).

### Symmetric 2D collapse (axis-order aware)
- `parse_axis_dimensions()` preserves real MDX axis order, not fixed metadata order.
- `map_pair_values()` interprets 2D SQL results by actual axis order.
- `ordered_pair()` builds tuples in `axis_dimensions` order.
- `ExcludedMember { dimension, key }`: dimension-tagged exclusions.
- `build_drilldown_member()` handles three collapse branches:
  1. Region collapse (excluded Produktkategori)
  2. Produktkategori collapse (excluded Produktkategori)
  3. Produktkategori collapse (excluded Region) — symmetric reverse case
- Collapse now works regardless of which dimension is above the other in Rows.

### ParsedMdx-driven classification
- `ParsedMdx` carries query-shape flags.
- `semantic_query_from_mdx()` classifies from struct, not `contains(...)` chains.

### Malloy-ready QueryPlan
- Typed `Dimension` / `Measure` / `TypedDimensionFilter`.
- `QueryPlan` with 4 variants: `Total`, `GroupBy`, `Count`, `Empty`.
- `execute_plan()` generates SQL from plan + model, generic backend execution.

### Malloy + SQL emitters
- `engine/malloy.rs`: static model + dynamic query emission.
- `engine/sql.rs`: SQL generation from typed plan.
- `engine/model.rs`: `SemanticModel` with `DimensionDef` / `MeasureDef` typed mappings.

### Criterion benchmarks + scale harness
- `benches/pipeline.rs`: `pipeline` (overhead) + `scale` (sizes) groups.
- `test_fixtures.rs`: shared MDX constants for tests and benches.
- Backend-injected execution: `execute_plan_with_backend()`, e2e variant.

### Earlier fixes
- `nom`-based MDX parser.
- DrilldownMember expand/collapse support (two forms).
- CrossJoin SlicerAxis ordering, complete SlicerAxis.
- Dimension-tagged filters, nested subquery filter merge.
- Collapsed All member Axis0 property fix.
- PARENT_UNIQUE_NAME omission.

## Project structure
```
xmla_proxy/
  Cargo.toml           — axum, duckdb (bundled), rusqlite, nom, criterion (dev)
  benches/
    pipeline.rs        — Criterion benchmarks (pipeline + DuckDB scale groups)
  src/
    lib.rs             — Crate root (lib+bin), re-exports all public modules
    main.rs            — Thin binary entrypoint
    test_fixtures.rs   — Shared MDX constants (tests + benches)
    parser.rs          — XmlaRequest parsing (quick-xml)
    response.rs        — SOAP envelope
    execute.rs         — Thin dispatch + test module (90 tests)
    execute_builders.rs — Cellset builders + flat-rowset fallback
    axis_members.rs    — Member/cell/axis/slicer helpers
    mdx_semantic.rs    — Semantic model, classification via ParsedMdx
    mdx_parser.rs      — nom-based MDX parser
    backend.rs         — DuckDB backend (singleton + benchmark generator)
    cellset.rs         — Cellset XML builder (mddataset)
    [metadata rowsets] — properties, schema_rowsets, catalogs, cubes, tables,
                         dimensions, hierarchies, levels, measures, members,
                         mdschema_properties, measure_groups, literals, sets,
                         kpis, tmschema, measuregroup_dimensions
    engine/
      mod.rs           — Module declaration
      plan.rs          — QueryPlan, QueryResult, Dimension, Measure,
                         plan_from_semantic(), execute_plan_with_backend()
      model.rs         — SemanticModel, DimensionDef, MeasureDef, default_model()
      malloy.rs        — Malloy emitter (model + query)
      sql.rs           — SQL emitter (model + query)
```

## What works
- Full discover handshake; Excel PivotTable works end-to-end.
- Single and two-dimension drilldown (CrossJoin).
- Cross-dimension filtering, slicer-only queries.
- **Symmetric expand/collapse on 2-hierarchy axis** — works regardless of row order.
- Probe queries: All.Members, All.Children, Leaf.Children, cChildren.
- `MDX -> ParsedMdx -> SemanticQuery -> QueryPlan -> {Malloy, SQL}` dual emission.
- Typed semantic IR: `Dimension`, `Measure`, `TypedDimensionFilter`, `ExcludedMember`.
- DuckDB backend as default execution engine.
- Criterion benchmarks for pipeline overhead + DuckDB scaling (10k-1M rows).
- Deterministic synthetic data generator with configurable profiles.
- 90 unit tests.

## What does not yet work
- Malloy is emitted but not compiled or executed at runtime.
- Full N-way MDX generalization.
- No `QueryPlan` caching layer yet.
- Some unused helper functions remain.

## Next workstreams (prioritised)

1. **QueryPlan caching.** **NEXT** — normalized `QueryPlan` cache for repeated
   Excel queries. Cache: Malloy text, SQL text, optionally recent results.
2. **Runtime Malloy compilation.** Evaluate once cache is in place.
3. **File-structure reorg.** Group modules: `mdx/`, `engine/`, `builders/`, `metadata/`.
4. **Remove stale code.** Clean up unused helpers and warnings.

### Completed
1. **ExecutionPlan → QueryPlan.** **DONE**
2. **Query-kind from parsed MDX.** **DONE**
3. **Split execute_builders.rs.** **DONE**
4. **Malloy generation.** **DONE**
5. **SQL generation + generic execution.** **DONE**
6. **Criterion benchmarks + scale harness.** **DONE**
7. **DuckDB backend.** **DONE**
8. **Symmetric 2D collapse.** **DONE**

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

19. **Malloy/SQL emission is effectively free** — sub-microsecond.

20. **Backend execution dominates at scale.** DuckDB is 5-10x faster than
    SQLite for grouped queries.

21. **XMLA rendering adds ~10-20% overhead** — not primary optimization target.

22. **Symmetric collapse requires dimension-tagged excluded members.**
    When rows are reversed, Excel sends Region members in the excluded set.
    The proxy must detect the excluded dimension and handle both collapse
    directions.

23. **2D SQL result column order must match `axis_dimensions`.** The builder
    must interpret SQL columns by visible axis order, not assume fixed
    `(kat, region)` positions.

## Hard-coded constants
- Catalog name: `KTH_KEX_MALLOY_CUBE`
- Cube name: `Model`
- Measure: `Total Försäljning` (caption `Total Försäljning (SEK)`)
- Measure group: `Faktatabell`
- Dimensions: `Produktkategori`, `Region`
- Session ID: `RUST-SESSION-456`

