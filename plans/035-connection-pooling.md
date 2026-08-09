# Plan 035: Connection pooling — concurrent DuckDB connections

## Status

- **Priority**: P1 (throughput, low effort)
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: performance

## Why this matters

DuckDB serializes all queries on a single connection — one `conn.query()` blocks
all others. With a single connection, concurrent Excel users contend for a
serial execution queue. A 10-user office running simultaneous PivotTable actions
would see 10× latency.

DuckDB connections are lightweight (MBs of RAM each). A pool of N connections
enables N concurrent queries with zero contention.

## Design

### Current state

`Backend::get()` returns a `&'static Backend` singleton wrapping a single
`Mutex<duckdb::Connection>`. Every query acquires this mutex, serializing all
work.

### Target

```rust
struct ConnectionPool {
    connections: Vec<Mutex<duckdb::Connection>>,
    next: AtomicUsize,
}

impl ConnectionPool {
    fn acquire(&self) -> PoolGuard<'_> {
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.connections.len();
        PoolGuard { guard: self.connections[idx].lock().unwrap() }
    }
}
```

Round-robin with no session affinity. Each `EXECUTE` request gets the next
connection in the pool. The DuckDB file is opened in read-only mode (no
wal/journal contention).

### Pool size

Default: `num_cpus` (typically 4–8). Configurable via env `DUCKDB_POOL_SIZE`.

### Guarantees

- **No deadlocks** — each request takes exactly one lock, releases before
  response. No nested lock acquisition.
- **No starvation** — round-robin ensures fair distribution.
- **No connection leaking** — `PoolGuard` drops the MutexGuard on response
  complete.

### Read-only mode

```rust
let config = duckdb::Config::default()
    .set("access_mode", "read_only")?;
conn.open_with_flags(path, config)?;
```

Read-only mode avoids write locks entirely. The proxy never writes to the user's
database.

## Scope

**In scope:**
- `ConnectionPool` struct with configurable size
- Read-only open mode
- Replace `Backend::get()` with pool-based `Backend::acquire()`
- Tests: verify concurrent queries don't serialize (timing-based assertion)

**Out of scope:**
- Write-path pooling (not needed — read-only workload)
- Connection health checks / reconnection (crash is fail-fast)
- Session affinity (not needed for stateless MDX)

## Done criteria

- [ ] N concurrent MDX queries execute in parallel (wall-clock time ≈
      serial time / N)
- [ ] Round-robin distribution verified (each connection handles roughly equal
      load)
- [ ] Read-only mode prevents any write contention
- [ ] All existing tests pass with pool semantics
