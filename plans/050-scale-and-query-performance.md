# Plan 050 — Scale and query performance: what a 50M-row model costs

Status: first measurement pass done 2026-09-23 (numbers below); follow-ups open.
Related: plan 031 (dimension dictionaries), plan 034 (streaming XML cellset,
deferred), plan 043 (RLS-aware rollup routing), plan 049 (Excel surface),
`.agents/skills/ssas-reference-oracle` (mirror shapes).

## Why this exists

Everything in plans 001–049 was verified against the demo model: 70k fact rows,
tiny dimensions. The question this pass answers is what happens when a user
points the proxy at a real mart — and which of the Excel gestures break first.

## Fixture

- `scripts/gen_bench_data.sh` with `ROWS=50000000`: 453 MB Parquet fact, a
  2020–2030 date dimension, exposed as views in one DuckDB file
  (`/home/felix/mallardcube-bench/`).
- Workload: `scripts/bench-workload.jsonl`, the captured Excel trace (9 Execute
  statements — Calendar and Full Date drilldowns at three depths, drillthrough,
  the no-axis total — plus 55 Discover requests).
- `scripts/bench.sh`, release build, 16 cores. Aggregations via
  `AGG=1` (`MALLARDCUBE_AGG_CACHE` sidecar).

## Measured: 50M rows, no aggregations vs rollups

`execute` = the 9 MDX statements, `discover` = the 55 metadata requests.

| workload | plain c=1 | plain c=8 | AGG=1 c=1 | AGG=1 c=8 |
|---|---|---|---|---|
| execute p50 | 23 ms | 743 ms | **4 ms** | **31 ms** |
| execute p90 | 220 ms | 1888 ms | 95 ms | 285 ms |
| execute p99 | 369 ms | 2732 ms | 101 ms | 460 ms |
| execute throughput | 9.9 req/s | 9.5 req/s | 64.3 req/s | **88.8 req/s** |
| discover throughput | — | 938 req/s | — | 896 req/s |
| discover p95 | — | 13 ms | — | 13 ms |

Single-statement, plain, sequential: no-axis total 46 ms, Calendar year
drilldown 215 ms, expanded year 319–367 ms, Full Date drilldown 305 ms,
drillthrough 98 ms, `MDSCHEMA_MEMBERS` 93 ms. Nothing has a cliff at 50M rows;
the rollups are a **9× throughput / 24× p50** win because every rollup carries
the date ancestors *and* every degenerate (fact-column) dimension, so the whole
Calendar/Full Date gesture set routes to it.

### The plain path saturates one query

Throughput is flat from c=1 to c=8 (9.9 → 9.5 req/s) while p50 goes 23 → 743 ms:
the machine is already at capacity with one 16-thread DuckDB query, so extra
concurrency is queueing, not waste. Pool size is 16 (`MALLARDCUBE_POOL_SIZE`),
each connection runs DuckDB's default 16 threads, so at c=8 the box is
oversubscribed ~8×. That is acceptable for a single-user proxy and is the
expected shape for a saturated analytical engine; rollups are the answer, not
thread tuning. Worth revisiting only if multi-user workloads become a goal.

## Measured: wide dimensions are the pivot-cache cliff

A 200k-member dimension (`/tmp/opencode/wide`, scratch fixture: 200k customers
joined to the 50M fact) with the restrictions *off* (the old behaviour) and on:

| request | before the fix | after | memory |
|---|---|---|---|
| `MDSCHEMA_MEMBERS` unrestricted | 237 MB, 200,044 rows, ~1.0–1.5 s | unchanged (correct) | proxy RSS 65 MB → 625 MB |
| `MDSCHEMA_MEMBERS` restricted to `[Category].[Category]` | 237 MB (restriction ignored) | **27 KB, 21 rows** | — |
| `DrilldownLevel` on the 200k hierarchy | 156 MB, 2.8 s cold / 1.1 s warm | unchanged | — |

So the member rowset is materialized and unbounded: cost is linear in the
hierarchy's cardinality, for both the response and the process. The dictionary
itself is fine (`SELECT DISTINCT` once, cached per dimension — plan 031); the
unbounded part is the XML. This is exactly plan 034's pragmatic scope
(incremental cell XML), extended to member rows; a response-size guard is the
alternative if streaming stays deferred.

## Finding: `MDSCHEMA_MEMBERS` ignored the Discover restrictions (fixed)

Excel builds a pivot cache one hierarchy (or level) at a time. The mirror
honours `DIMENSION_UNIQUE_NAME`, `HIERARCHY_UNIQUE_NAME`, and
`LEVEL_UNIQUE_NAME`; the proxy dropped them on the floor (`MdschemaMembers`
carried only the member probe and `TREE_OP`), so *every* restricted request
answered with every hierarchy of every dimension — a 237 MB response on the
wide fixture where 27 KB is correct. `coordinates_match` already existed for
this (plans 048/049); the member rowset is now wired through it.

Mirror (SQL Server 2025 tabular, `MallardDemo`) vs proxy after the fix, same
requests:

| restriction | mirror | proxy |
|---|---|---|
| `[Date].[Calendar]` | 4206 rows | 4206 rows |
| `[Date].[Calendar].[Year]` | 11 rows | 11 rows |
| `[Date].[Calendar].[Quarter]` | 44 rows | 44 rows |
| `[Date].[Calendar].[Month]` | 132 rows | 132 rows |
| `[Category].[Category]` | 21 rows | 21 rows |
| member + `TREE_OP` 8, with and without a matching hierarchy | 1 row | 1 row |
| `[Date].[Full Date]` | 4019 rows | **0 rows — gap** |
| `[Measures]` | 6 rows | **0 rows — gap** |

The two gaps are pre-existing (not caused by the restriction fix), but the fix
makes them visible: a client that asks for the date key hierarchy or the
measures hierarchy now gets an empty rowset instead of a wrong one. Mirror
shapes, for whoever picks these up:

- `[Date].[Full Date]`: All at `[Date].[Full Date].[(All)]` (`MEMBER_TYPE` 2,
  `CHILDREN_CARDINALITY` 4018, `MEMBER_KEY` 0) plus 4018 leaves at
  `[Date].[Full Date].[Full Date]` (`MEMBER_TYPE` 1, unique name
  `[Date].[Full Date].&[2020-01-01T00:00:00]`, `MEMBER_KEY` `1/1/2020`,
  locale-formatted `MEMBER_NAME`).
- `[Measures]`: one row per measure, hierarchy `[Measures]` (no dot), level
  `[Measures].[MeasuresLevel]`, `MEMBER_TYPE` 4, unique name
  `[Measures].[Revenue]`, `CHILDREN_CARDINALITY` 0.

Compound member unique names also differ by convention — the mirror nests
(`[Date].[Calendar].[Year].&[2020].&[1]`), the proxy qualifies by level
(`[Date].[Calendar].[Quarter].&[2020]&[1]`). Both are self-consistent (the
axes, the member rowset, and the probes agree inside each engine) and Excel
works with the proxy's form across every verified gesture, so this is a note,
not a defect.

## Finding: a degenerate high-cardinality column inflates every rollup

`design_aggregations` groups each rollup by the date ancestors *plus every
degenerate (fact-column) dimension*, whatever its cardinality. Measured on the
wide fixture with a 200k-member degenerate key:

| rollup (coarsest, year grain) | rows |
|---|---|
| with the 200k-member column | **12,582,327** |
| without it | 35,200 |
| fact table | 50,000,000 |

One such column turns each rollup into ~25% of the fact (and the sidecar into
several times the source), while contributing nothing usable: nobody pivots a
200k-member attribute through a rollup. A relationship-backed dimension is
already excluded from leaf columns, so this needs a *degenerate* column
(invoice ids, customer ids in flat models — exactly what the AutoModel path
produces). Suggested guard: drop leaf columns whose distinct count is more than
a small fraction of the fact row count when building the sidecar, and record
that in `Aggregation::leaf_columns` so `agg_covers` falls back to the fact for
queries grouping by them.

## Harness fixes (this pass)

- `load_replay::validate_response` accepted only cellsets, so DRILLTHROUGH
  (a rowset) counted as a failure — a phantom 10–11% error rate on a workload
  the proxy answered correctly. Rowsets are now accepted.
- `scripts/bench.sh` aborted after its first block: the replay tool exits
  non-zero when its own error-rate threshold trips, and `set -e` treated that
  as fatal, skipping the aggregation comparison entirely. The sweep now
  reports the numbers and continues.

## Open items

1. Wide-hierarchy member listings: incremental XML writing (plan 034) or a
   size guard, with the 200k-member fixture as the benchmark.
2. `[Date].[Full Date]` and `[Measures]` member rows (shapes above).
3. Rollup leaf-column cardinality guard (measurement above).
4. Rollup build time and sidecar size on a wide model (not measured here: the
   50M-row build was fast enough that the harness never printed its wait
   message; worth a number once item 3 changes the design).
