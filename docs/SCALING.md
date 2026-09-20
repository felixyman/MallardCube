# Scaling notes

Measured numbers for a large fact table, so "will it hold up?" has an answer
with a reproduction path. These are single-machine numbers on synthetic data —
use them for shape and ratios, not as a capacity promise.

## Dataset and harness

- **Fact**: 100,000,000 rows, Parquet-backed DuckDB view (`sales_fact`),
  906 MB Parquet + a 268 KB DuckDB file with views (`scripts/gen_bench_data.sh`).
- **Date dimension**: 2020–2030 calendar (36 KB Parquet).
- **Workload**: `scripts/bench-workload.jsonl` — a captured Excel session:
  a total, date-hierarchy drilldowns (`DrilldownLevel`), multi-parent
  expansions (`DrilldownMember`), and their `CELL PROPERTIES` variants, plus
  the discover handshake. Replayed by `mallard load-replay`.
- **Machine**: 16 cores, 31 GB RAM, NVMe (the Parquet files live on NVMe; the
  proxy's pool defaults to one read-only connection per core, capped at 32).
- **Reproduce**:

  ```bash
  ROWS=100000000 BENCH_DIR=/path/on/nvme bash scripts/bench.sh            # baseline
  AGG=1 ROWS=100000000 BENCH_DIR=/path/on/nvme bash scripts/bench.sh      # rollups
  BENCH_DIR=/path/on/nvme bash scripts/rls-rollup-ab.sh                   # secured query A/B
  ```

## Results (result cache off, so every request executes)

| Mode | execute, 1 client | execute, 8 clients | discover, 8 clients |
|---|---|---|---|
| Fact scans (no rollups) | 2.07 req/s · p50 **431 ms** · p95 733 ms | 2.18 req/s · p50 **3 596 ms** · p95 6 028 ms | 547 req/s · p50 0 ms |
| Rollups (`MALLARDCUBE_AGG_CACHE`) | 163.7 req/s · p50 **4 ms** · p95 10 ms | 870.4 req/s · p50 **8 ms** · p95 16 ms | 606 req/s · p50 0 ms |

- **~80× throughput** at one client and **~400×** at eight: rollups turn a
  100M-row scan into a few thousand rows. Concurrency *helps* with rollups
  (the connection pool spreads them) and *hurts* without (full scans contend
  for memory bandwidth).
- With the 5-second result cache enabled (Excel's repeated
  `CELL PROPERTIES` variants), the same runs measure 321 req/s and 1 662 req/s
  respectively — real Excel traffic gets both effects.
- **Discover is unaffected by fact size** (447–606 req/s): member dictionaries
  are cached per dimension (plan 031).

## Row-level security

Rollups are built from the full fact, so a role predicate must be applied to
the rollup (plan 043). For a role that filters `territory = 'North'`
(1/8 of the fact):

| Path | Query time (3 runs) | Value |
|---|---|---|
| Fact scan (predicate not rollup-expressible) | 0.193 s · 0.124 s · 0.122 s | 324 924 070 034 |
| Rollup (predicate applied to the rollup) | 0.0018 s · 0.0015 s · 0.0014 s | 324 924 070 034 |
| Raw SQL oracle | — | 324 924 070 034 |

Same number, ~80× faster. Predicates the rollup cannot evaluate (unrolled fact
columns, deeper date levels, other aliases, subqueries) keep the fact path —
never a silent bypass.

## Startup

- Building the rollup sidecar scans the fact once: **8 s** for 100M rows on
  this machine (the sidecar is 6 MB).
- Later starts reuse the sidecar when its stamp (source size + mtime) matches:
  under a second.
- A data reload (SIGHUP) with changed data disables rollups until the next
  restart; queries fall back to the fact table and the log says so (plan 041).

## Known limits

- Rollups cover additive `SUM` measures only; `COUNT`, `MIN`, `MAX`,
  expressions, and time-intelligence measures scan the fact.
- Leaf-level drilldowns (per-day, per-customer) scan the fact by design.
- A leaf-level crossjoin of two large dimensions materialises every tuple in
  memory (streaming XML is deferred, plan 034); cap the axis in Excel.
- There is no query timeout yet: a pathological pivot occupies one pool
  connection until it finishes.
