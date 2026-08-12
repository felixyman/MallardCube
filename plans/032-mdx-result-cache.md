# Plan 032: MDX-hash result cache — deduplicate repeated Excel queries

## Status

- **Priority**: P1 (perf, low effort)
- **Effort**: XS
- **Risk**: LOW
- **Depends on**: none
- **Category**: performance

## Why this matters

Excel sends 3 nearly-identical MDX statements per PivotTable interaction — one
for `CELL PROPERTIES VALUE`, one for `CELL PROPERTIES FORMAT_STRING, BACK_COLOR,
FORE_COLOR`, and one for `CELL PROPERTIES CELL_ORDINAL`. All three produce the
same DuckDB query result. Currently, MallardCube re-executes the same SQL 3
times.

A short-lived in-memory cache on the MDX hash eliminates 2 out of 3 queries per
user action.

## Design

### Cache key

Normalize the MDX before hashing: strip cell-property clauses, normalize
whitespace. Two MDX statements that differ only in `CELL PROPERTIES` should map
to the same cache key.

```rust
fn cache_key(mdx: &str) -> u64 {
    let normalized = strip_cell_properties(mdx);
    // deterministic hash of normalized MDX
}
```

### Cache entry

```rust
struct CachedResult {
    rows: Vec<Vec<(String, f64)>>,  // axis × cell values
    col_names: Vec<String>,
    inserted_at: Instant,
}
```

### TTL

5 seconds. Excel's three requests arrive within 50-100ms of each other. A TTL
longer than that gives no benefit and only wastes memory against a burst of
different queries.

### Placement

In `src/execute/runtime.rs`, before `get_execute_cellset_response_with_backend`.
Wrap the plan→SQL→execute chain:

```rust
if let Some(cached) = RESULT_CACHE.get(&key) {
    if cached.fresh() {
        return render_from_cached(cached, mdx);
    }
}
let result = execute_and_render(mdx, backend);
RESULT_CACHE.insert(key, result.clone());
```

### Concurrency

`RESULT_CACHE` is a `LazyLock<Mutex<LruCache<u64, CachedResult>>>` with a small
cap (64 entries). The mutex is held only for the cache lookup/insert, not during
DuckDB execution. Multiple concurrent queries with different MDX don't contend.

## Scope

**In scope:**
- `SemanticResult` wrapper with cell values + axis metadata sufficient to render
  all 3 cell-property variants
- Normalize MDX before hashing (strip `CELL PROPERTIES` clause)
- 5-second TTL, 64-entry cap, per-request ttl check
- Tests: verify that 3 identical-data MDX variants only hit DuckDB once

**Out of scope:**
- Cross-request durable cache (data changes between requests)
- Query-plan-level cache (plan construction is cheap; SQL execution is expensive)
- Cache invalidation on INSERT/UPDATE (read-only workload)

## Done criteria

- [ ] 3 identical-MDX-result variants in rapid succession → DuckDB executes once
- [ ] Different MDX → different cache keys → no false hits
- [ ] Cache entry older than TTL → re-executes
- [ ] Existing cellset tests pass unmodified
