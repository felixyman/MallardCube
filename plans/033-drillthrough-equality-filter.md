# Plan 033: DRILLTHROUGH equality filter — replace CAST+LIKE with direct equality

## Status

- **Priority**: P1 (correctness + perf, tiny effort)
- **Effort**: XS
- **Risk**: LOW
- **Depends on**: none
- **Category**: performance

## Why this matters

DRILLTHROUGH currently builds WHERE clauses using `CAST(col AS VARCHAR) LIKE
'value%'`. This is non-sargable — it forces a full table scan on every
drillthrough request, even when the column has the exact value being filtered.

For a 100M-row fact table, a DRILLTHROUGH with 3 slicer filters scans 100M rows
instead of seeking to the matching rows.

## Design

### Current code (`src/execute/dispatch.rs:65-66`)

```rust
where_clauses.push(format!(
    "CAST({col} AS VARCHAR) LIKE '{}%'",
    k.replace('\'', "''")
));
```

### Fix

Use equality when the column is a dimension key (integer FK):

```rust
if col_is_integer_fk(model, dim, col) {
    where_clauses.push(format!("{col} = {}", k.replace('\'', "''")));
} else {
    where_clauses.push(format!("CAST({col} AS VARCHAR) = '{}'", k.replace('\'', "''")));
}
```

For string columns, keep CAST but use `=` instead of `LIKE`. For integer FK
columns (the common case — date_key, territory_id), use direct equality without
CAST.

### Column type detection

`dim.physical_field` tells us the dimension column type. DuckDB's
`PRAGMA table_info('table')` gives the SQL type at runtime, or we can infer from
the fact column's type: if the relationship's `fact_column` is INTEGER on the
fact table, it's an FK and can use `=` directly.

Simplest: always use `=` instead of `LIKE`. The `%` suffix was never correct
semantics for DRILLTHROUGH — the slicer has an exact member key, not a prefix.
The `LIKE 'value%'` would match `value1`, `value2` etc. which is a bug, not just
a performance issue.

## Scope

**In scope:**
- Replace `CAST(col AS VARCHAR) LIKE 'key%'` with `col = key_value` for integer
  FKs
- Replace with `CAST(col AS VARCHAR) = 'key'` for string columns
- Preserve SQL escaping (`''` for single quotes)

**Out of scope:**
- Full schema type caching (use relationship metadata + fallback to CAST)

## Done criteria

- [ ] Integer FK DRILLTHROUGH uses `col = 1234` (no CAST, no LIKE)
- [ ] String column DRILLTHROUGH uses `CAST(col AS VARCHAR) = 'value'` (no LIKE)
- [ ] Existing DRILLTHROUGH tests pass (Contoso + project3)
- [ ] Verify with `EXPLAIN` that the query plan uses an index/scan on the FK
      column, not a full-table scan
