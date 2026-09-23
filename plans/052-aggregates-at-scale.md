# Plan 052 — Aggregates at scale: cardinality-aware design, more measures, and who builds them

## Status

- **Priority**: P1 (this is what makes big facts fast or slow)
- **Effort**: L
- **Risk**: MEDIUM (rollup contents feed query routing; a bad rollup must
  degrade, never answer wrong)
- **Depends on**: 043 (RLS-aware routing), 050 (measurements)
- **Related**: 054 (engine readiness — materialisation must stay engine-neutral)
- **Category**: performance

## Why this matters

Rollups are the reason a 50M-row model answers Excel in 4–31 ms instead of
23–743 ms (plan 050). Three limits show up as soon as the model gets big:

1. **The design groups by every degenerate column.** `design_aggregations`
   (`src/engine/aggregate.rs:97`) adds every relationship-less dimension to
   every rollup. Measured with a 200k-member degenerate column: the coarsest
   rollup grows from **35,200 to 12,582,327 rows** (25% of the fact) while
   contributing nothing usable — nobody pivots 200k members through a rollup.
   On a 1B-row fact that is a sidecar larger than the source.
2. **Only plain `SUM(col)` is rollup-able.** `measure_base_column` returns
   `None` for anything else and time-flagged measures are skipped entirely, so
   `COUNT`, `AVG`, `MIN`/`MAX` and every time-intelligence measure fall back to
   scanning the fact — at 1B rows that is the difference between milliseconds
   and tens of seconds.
3. **The sidecar is built synchronously before serving** (`ensure_aggregations`
   at startup). A full scan of a 1B-row fact is minutes of "not ready", and the
   stamp (source size + mtime) rebuilds all of it on any upstream change.

## Design

### A. Cardinality-aware leaf columns

At build time (`ensure_aggregations`, where a connection to the source
exists), measure each degenerate column's distinct count — one combined
`SELECT COUNT(DISTINCT c1), COUNT(DISTINCT c2), ...` over the fact, or the dim
dictionary when it is already warm — and **drop columns above a threshold**
from `Aggregation::leaf_columns`:

- `MALLARDCUBE_AGG_MAX_LEAF_CARDINALITY` (default to be chosen from a 500M-row
  measurement; the 12.6M-row result above is the argument for ~10^5, or a small
  fraction of fact rows).
- Routing needs no change: `agg_covers` already falls back to the fact when a
  group-by dimension is not in the rollup. Skipped columns are logged and
  visible in `/status` so the behaviour is discoverable.
- Tests: a high-cardinality degenerate column is excluded; a low-cardinality
  one is kept; sibling values still match the fact.

### B. Measure specs beyond SUM

Replace `measure_base_column` with an enum that describes how the rollup stores
a measure, and teach the builder + validator + routing about it:

| measure expression | rollup columns | rollup aggregate |
|---|---|---|
| `SUM(x)` | `x` | `SUM(x)` |
| `COUNT(x)` | `n_x` | `SUM(n_x)` |
| `COUNT(*)` | `n` | `SUM(n)` |
| `MIN(x)` / `MAX(x)` | `x` | `MIN(x)` / `MAX(x)` |
| `AVG(x)` | `sum_x`, `n_x` | `SUM(sum_x)/SUM(n_x)` |

Ratio-of-sums measures already resolve at the plan level; keep that path.
Time-intelligence measures stay out of scope here (plan 046 owns their
semantics) except where they are additive over a date range — those route
through the date ancestors already present in the rollup.

Validation (`validate_rollups`) must cover each spec, not just SUM totals:
compare each rollup measure against the fact with the same tolerance rule.

### C. Never block serving on a build

- `ensure_aggregations` moves off the startup path: serve from the fact while
  the sidecar builds, then swap in via `set_aggregations` (already exists for
  the plan 041 reload path).
- `/status` gains `aggregations: { state: absent|building|ready|stale, tables,
  built_at, source_stamp }`; `REFRESH CUBE` can trigger a rebuild (currently a
  no-op success by design).
- Incremental refresh: rebuild only the rollups whose source partition changed
  when the fact is date-partitioned; otherwise a full rebuild is acceptable as
  long as it is off the request path.

### D. Who builds the aggregates (the architectural fork)

Small deployments: the proxy builds the sidecar (today's behaviour, now
non-blocking). Large deployments: aggregates belong upstream (sqlmesh/DuckDB
marts), consistent with "definitions live upstream". Make that first-class:

- Config-declared upstream aggregates, e.g.
  `"aggregations": { "mode": "upstream" }`, meaning: tables named by the
  documented convention (`agg_<level>`, same columns as this plan's builder
  produces) plus the existing stamp table are read from the source database,
  validated with `validate_rollups`, and routed without any build.
- `mode: "proxy"` (default) keeps today's behaviour; `mode: "upstream"` never
  writes to the source.
- **Aggregates are artifacts, not state.** They are versioned (the manifest
  carries format version, grain, covered measures and a source fingerprint),
  can travel as tables in the source database, Parquet/Iceberg, or a DuckDB
  file, and can be mounted read-only where the database is not writable — so
  replicas are stateless, a rolling update never reruns a build, and no
  serve-time write credential is needed.
- Portable plain tables first (they work on every engine). Engine-specific
  accelerators — ClickHouse `AggregatingMergeTree` / projections, incremental
  materialized views — are a documented recipe later, behind the same manifest
  and the same validation; they must never become a requirement for using
  aggregates.
- Document the decision matrix (data size, freshness, deployment count) on the
  Aggregations docs page, and give sqlmesh users a matching SQL recipe.

## Scope

**In:** cardinality guard, measure specs, non-blocking build, `/status`,
upstream-declared aggregates, docs + tests.

**Out:** distributed execution, MDX feature growth, rollups for arbitrary
calculated members, write paths, query-plan routing beyond `agg_covers`.

## Done criteria

- Wide fixture: a 200k-member degenerate column is excluded from every rollup;
  the sidecar stays under a documented size; the coarsest rollup is 35k rows,
  not 12.6M.
- A model with `AVG`, `COUNT`, `MIN`, `MAX` measures routes to rollups and
  matches the fact within tolerance; oracle corpus unchanged.
- Startup serves within the existing readiness window with the sidecar absent,
  then `/status` transitions to `ready` without a restart.
- `mode: "upstream"` answers the same bench workload with no writes to the
  source; `validate_rollups` rejects a tampered table.

## STOP conditions

- If a measure spec cannot be validated against the fact with the existing
  tolerance rule, leave that measure on the fact path rather than serving an
  approximate rollup.
- If upstream mode needs source-database write access for anything but the
  sidecar, stop: that violates the read-only boundary.
