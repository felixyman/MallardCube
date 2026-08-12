# Plan 034: Streaming XML cellset render — constant memory for large crossjoins

## Status

- **Priority**: P2 (memory, medium effort)
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: performance

## Why this matters

A large PivotTable crossjoin (e.g. 10 categories × 50 regions = 500 cells) is
fine. But Excel can legally request a crossjoin of 10K × 10K = 100M cells. The
current renderer builds the entire XML cellset as a `String` in memory before
writing to the response. For a 100M-cell response, that's several GB of XML in
RAM before the first byte reaches the client.

Streaming the XML cellset allows unbounded crossjoin sizes with constant memory.

## Design

### Current path

```
plan → SQL → DuckDB → Vec<(String, f64)> → build XML String → HTTP response
```

### Target path

```
plan → SQL → DuckDB → RowIter → streaming XML writer → HTTP response
```

### Implementation

Axum supports streaming responses via `axum::body::Body::from_stream`. Instead
of returning a `String`, return a `Stream` of XML chunks.

The cellset XML is a prefix (axes, hierarchies, members), a middle (cell data),
and a suffix (closing tags). The prefix and suffix are small and non-streamable
(the member list is bounded by dimension cardinality). The cell data is
unbounded.

```rust
struct StreamingCellset {
    prefix: String,       // <Axes> ... </Axes> <CellData>
    cell_values: Vec<f64>, // from DuckDB (not row-by-row — DuckDB returns all at once)
    suffix: String,       // </CellData> </root> ...
}
```

### The real win: DuckDB-side

DuckDB returns all rows in one batch — there's no streaming cursor. The memory
for the DuckDB result vector is the real bottleneck, not the XML string.

**Fix**: Use `LIMIT` + pagination for large grouped queries, or trust DuckDB's
memory efficiency (100M rows of `(String, f64)` is ~2.4 GB — high but not
catastrophic on a 16 GB server).

For the XML side: incremental cell XML writing reduces the copy overhead (no
second full copy of the cellset XML in memory alongside the DuckDB result).

### Pragmatic scope

Don't build a full streaming XML writer. Just write cells incrementally instead
of `collect::<String>()`:

```rust
let mut buf = String::with_capacity(cells.len() * 128);
buf.push_str(CellData::PREFIX);
for (i, (cell, vals)) in cells.iter().enumerate() {
    write!(&mut buf, "<Cell CellOrdinal=\"{}\"><Value>{}</Value></Cell>", i, vals.0);
}
buf.push_str(CellData::SUFFIX);
```

This is a small refactor — replace `format!` accumulation with a `String` buffer
push. The memory is dominated by the DuckDB result vector, which is already
allocated.

## Scope

**In scope:**
- Replace `collect::<Vec<String>>()` + `join("\n")` pattern in cellset renderer
  with incremental `write!` to a pre-allocated `String`
- Measure memory reduction on a 1000-cell crossjoin

**Out of scope:**
- True streaming (DuckDB cursor-based row iteration) — DuckDB API doesn't support it
- Axum streaming response body — the memory win is already in XML construction
- LIMIT-based pagination — not needed for current segment

## Done criteria

- [ ] Cellset XML construction uses incremental writes, not intermediate
      `Vec<String>`
- [ ] Memory profile for 100K-cell render is measurably lower (benchmark)
- [ ] All existing cellset shape tests pass
