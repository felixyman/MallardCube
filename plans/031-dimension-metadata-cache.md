# Plan 031: In-memory dimension metadata cache — eliminate N+1 metadata queries

## Status

- **Priority**: P2 (performance, not correctness)
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: performance

## Why this matters

Every PivotTable refresh issues N extra `COUNT(DISTINCT)` queries for slicer-axis
ALL members. Every `MDSCHEMA_MEMBERS` request issues 2N `SELECT DISTINCT` queries
(for N dimensions) before applying the member filter. Hierarchy expands issue 2
redundant queries. These queries are fast individually (~5-10ms in DuckDB) but
compound per user interaction, producing a perceptible latency gap between
MallardCube and SSAS (which pre-builds all member dictionaries at process time).

The fix is to pre-build the same dictionaries at `ProxyProject::load()` time and
serve metadata requests from in-memory cache. This eliminates ALL metadata
queries from the hot path.

## Current query sites (all cacheable)

| Site | File:line | SQL | Freq |
|---|---|---|---|
| Slicer ALL cardinality | `axis_members.rs:45` | `COUNT(DISTINCT col) FROM dim_table` | Per visible non-axis dim per refresh |
| ALL member cardinality | `members.rs:78` | Same | Per dim per discover |
| Leaf member values | `members.rs:135` | `SELECT DISTINCT col FROM dim_table ORDER BY col` | Per dim per discover |
| Level children (count) | `members.rs:303` | `COUNT(DISTINCT child_col) FROM table WHERE parent_col = 'k'` | Per hierarchy expand |
| Level children (values) | `members.rs:313` | `SELECT DISTINCT CAST(child_col AS VARCHAR) FROM table WHERE parent_col = 'k'` | Per hierarchy expand |

## Design

### New type: `DimensionCache`

```rust
// In src/engine/model.rs

struct DimMemberCache {
    leaf_values: Vec<String>,
    all_cardinality: u32,
}

struct LevelChildrenCache {
    parent_to_children: HashMap<String, Vec<String>>,
}

struct DimensionCache {
    leaf: DimMemberCache,
    levels: Vec<LevelChildrenCache>,
}

// In SemanticModel
dimension_cache: HashMap<String, DimensionCache>,
```

### Build at load time

In `build_semantic_model()` (or a post-build step), for each dimension:

1. `SELECT DISTINCT col FROM dim_table ORDER BY col` → `leaf_values`
2. `SELECT COUNT(DISTINCT col) FROM dim_table` → `all_cardinality`
3. For each level in a multi-level hierarchy: `SELECT DISTINCT parent_col, child_col FROM dim_table` → `parent_to_children` map

For dimensions with large `cardinality_hint` values (>50K), skip caching
level-children maps and keep the query fallback.

### Query site changes

`dim_children_count` (`axis_members.rs:40`): delete; caller reads `DimensionCache.leaf.all_cardinality`.
`build_all_member_rows` (`members.rs:69-84`): read from cache instead of running COUNT.
`build_leaf_member_rows` (`members.rs:135-144`): read `leaf_values` from cache.
`query_level_children` (`members.rs:278`): read `parent_to_children` map from cache; `child_count = children.len()` instead of separate query.

Remove `QueryBackend` parameter from `dim_children_count`, `dim_props_all`, `all_member_for_dim`, `build_all_member_rows`, `build_leaf_member_rows` — they no longer need a database handle.

### Filtered access (RLS)

Dimensions with `TableAccess::Filtered` need their values filtered by the
user's role predicate. Cache stores unfiltered values; apply the predicate
as a post-filter on the cached `Vec<String>` (the predicate is a simple SQL
WHERE clause — for caching, we'd need to parse it or execute it once at
cache-warm time per role). Simplest approach: store unfiltered cache and
apply `StrFilter` on retrieval.

### Memory estimate

| Model | Dims | Largest dim | Leaf values (est.) | Level maps (est.) | Total |
|---|---|---|---|---|---|
| project3 (demo) | 2 | ~10 | <1 KB | 0 (no levels) | <1 KB |
| generated_retail | 8 | ~100 | ~5 KB | 0 | ~5 KB |
| generated_project | ~50 | ~100 | ~50 KB | 0 | ~50 KB |
| Contoso | 11 | Customer (105K) | ~2 MB | ~1 MB | ~15 MB |
| Worst hypothetical | 10 dims × 100K | 100K | ~20 MB | ~10 MB | ~100 MB |

All comfortably within a departmental server's memory budget. For reference,
SSAS Tabular's VertiPaq engine uses 2-5× more memory for the same data.

## Query count impact

| User action | Before cache | After cache |
|---|---|---|
| PivotTable refresh | 1 data + 3–6 metadata | **1 data** |
| Field list open (10 dims) | 20+ queries | **0** |
| Hierarchy expand (multi-level) | 2N + 2 queries | **0** |
| DRILLTHROUGH | 2 queries | 1 (pragma merge) |

## Scope

**In scope:**
- `DimensionCache` type in `src/engine/model.rs`
- Build cache at project load time (`build_semantic_model` or post-load step)
- Replace `dim_children_count`, `build_all_member_rows`, `build_leaf_member_rows` with cache reads
- Replace `query_level_children` queries with cache reads (for dims under the cardinality threshold)
- Remove `QueryBackend` dependency from affected functions
- Update tests (mock/demo data must populate cache)

**Out of scope:**
- RLS-filtered cache (store unfiltered; filter on retrieval — follow-up if needed)
- DRILLTHROUGH pragma query merge (separate micro-optimization)
- Dynamic cache refresh (stale data between restarts is acceptable — same as SSAS)
- Caching for non-dimension metadata (measures, cubes are static)

## Risks

- **Startup latency**: 100-500ms for large models. Acceptable; SSAS processing takes minutes.
- **Large-dimension cache**: Dimensions with >50K members get leaf-only cache (no level maps); hierarchy expands still query DuckDB. This is the Contoso Customer dimension case.
- **Memory**: 100 MB worst case for an unrealistic 10-dim × 100K-member model. Realistic worst case (Contoso) is 15 MB.

## Test plan

- Verify that after cache warm, metadata functions return identical results to the live-query versions (snapshot tests on `axis_members`, `discover/members`)
- Verify `SELECT DISTINCT` and `COUNT(DISTINCT)` are never called on the metadata path
- Run full test suite — no regressions in PivotTable cellset shape, discover response shape

## Done criteria

- [ ] All metadata query sites read from `DimensionCache` instead of DuckDB
- [ ] `QueryBackend` parameter removed from `dim_children_count`, `dim_props_all`, `all_member_for_dim`, member builders
- [ ] Cache populated at load time; startup logs dimension counts
- [ ] All existing tests pass unmodified (or trivially adapted to cache semantics)
- [ ] Manual Excel test: field list opens instantly, hierarchy expand is instant
