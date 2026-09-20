# Design invariants — a protocol adapter, not a semantic layer

MallardCube is the **Excel/XMLA edge for modern SQL stacks**. Metric logic lives
upstream (sqlmesh/dbt models, or a semantic layer); MallardCube translates MDX,
speaks Excel's dialect of XMLA, and serves a mechanical projection of what
upstream already materialised.

This document is the contract. It exists because the alternative — growing a
measure language inside the proxy — makes the product harder to adopt (metrics
defined twice) and impossible to defend ("why is my business logic in a proxy
config?"). See [plan 044](../plans/044-boundary-contract.md).

## What MallardCube owns

- The XMLA/MDX wire protocol: SOAP envelopes, `DISCOVER`/`MDSCHEMA_*` rowsets,
  cellset XML, sessions, `CELL PROPERTIES`.
- Excel shape fidelity: axis conventions, drill/collapse, `DISPLAY_INFO`,
  member properties, `MEMBER_KEY`, `DRILLTHROUGH`.
- Delivery mechanics: connection pool, short-lived result cache, hot reload,
  `/health` and `/status`.
- A **mechanical projection** of an upstream model onto SQL.

## What it does not own

- Measure definitions beyond `SUM(column)` and plain SQL over exposed tables.
- DAX evaluation, calculation groups, measure dependency graphs.
- Time-intelligence functions. The date-flag contract is the model example:
  `ytd_flag` is an upstream column, the proxy only filters on it.
- Join semantics beyond declared relationships.
- Pre-aggregation semantics. The rollup sidecar is a cache; if the upstream
  already pre-aggregates, turn `MALLARDCUBE_AGG_CACHE` off.

## The five invariants

| # | Invariant | Enforcement |
|---|---|---|
| 1 | Measures are `SUM(col)` or plain SQL over exposed tables | `mallard qualify --strict` fails on fallback SQL and reports non-additive measures |
| 2 | No DAX | the converter emits a "define upstream" checklist instead of growing its lowering patterns; `sql_fallback` is labelled bridge code, frozen apart from bug fixes |
| 3 | Flags and definitions live upstream | the date-flag contract (`ytd_flag` etc.) is the worked example |
| 4 | Rollups are a cache, never semantics | they accept only `SUM(col)`; docs tell teams to disable them when the upstream pre-aggregates |
| 5 | Feature intake rule | any request that would add measure-level logic is answered with "do it upstream" plus a recipe (below), not with new proxy features |

## What belongs upstream (generic example)

An orders domain with status flags, waiting-time buckets, and cumulative
metrics. The upstream (sqlmesh/dbt) models materialise:

1. **A conformed fact with additive columns.** Counts become 0/1 flags
   (`is_open`, `is_cancelled`, `is_late`); durations become a sum/count pair
   (`sum_lead_time`, `count_lead_time`); ratios become two additive components.
   The proxy measures are then `SUM(flag)` and `SUM(a) / SUM(b)`.
2. **Per-grain marts for non-additive metrics.** Medians and percentiles are
   materialised at a declared grain (for example month × product). A yearly
   "median of monthly medians" is not a median, so the pivot flexibility is
   pinned to that grain — a modelling decision, made explicitly.
3. **Cumulative snapshots.** Year-to-date and prior-year cumulative values are
   window functions upstream, materialised per month. Because cumulative values
   are monotonic within a year, the period-end value is the correct value at
   coarser grains.
4. **Conformed dimensions** with keys, labels, and hierarchies. Facts that share
   a dimension use the same join column name, so one relationship per dimension
   serves them all.

The runnable proof lives in [`projects/upstream_marts/`](../projects/upstream_marts/README.md):
schema, seed, and marts SQL plus a thin projection whose every measure is plain
SQL. `qualify --strict` passes there and fails on a project that carries
DAX-derived fallback SQL.

## Honest trade-offs

- **Grain pinning** (non-additive metrics) is the price of keeping the proxy
  thin. If the business needs the same metric at another grain, materialise
  another mart or accept a fact scan.
- **RLS policy vs enforcement**: the policy is upstream (role → SQL predicate);
  enforcement is at the edge (trusted header → injected predicate).
  Column-level masking is the known gap; per-role views upstream are the
  documented answer.
- **No automatic pre-aggregation matching**: marts are explicit; the rollup
  cache is the fallback for stacks without marts.

## Non-goals

No DAX engine, no calculation groups, no time-intelligence functions, no
measure-of-measure evaluation beyond plain SQL, no multi-dialect SQL emitter,
no attempt to replicate a headless-BI feature set, no growth of `sql_fallback`
beyond bug fixes.
