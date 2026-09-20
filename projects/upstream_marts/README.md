# Upstream marts — a thin projection, not a semantic layer

This project is the runnable proof of [plan 044](../../plans/044-boundary-contract.md):
the metric logic lives **upstream** in SQL models, and MallardCube is a thin
projection of what they materialised. There is no DAX, no fallback SQL, and no
measure logic in the proxy config.

## Layout

| Path | What it is |
|---|---|
| `upstream/schema.sql` | Conformed dimensions and an additive fact (`fact_orders`) |
| `upstream/seed.sql` | Deterministic demo data (no future dates; flags vs `CURRENT_DATE`) |
| `upstream/marts.sql` | Per-grain marts: monthly × product medians, monthly cumulative revenue |
| `build.sh` | Builds `data/upstream_marts.duckdb` from the three files |
| `proxy-config.yaml` | The thin projection (every measure is plain SQL) |

In a real stack these SQL files would be sqlmesh/dbt models with tests,
lineage, and environments — the point is that they are *yours*, versioned next
to the warehouse, not hidden in a proxy config.

## Run it

```bash
bash projects/upstream_marts/build.sh
PROXY_CONFIG=projects/upstream_marts/proxy-config.yaml cargo run
# Excel: connect to http://localhost:8080/xmla, catalog UPSTREAM_DEMO, cube Orders
```

Prove the boundary (CI-usable):

```bash
cargo run --bin mallard -- qualify --strict projects/upstream_marts/proxy-config.yaml
```

## What each measure demonstrates

| Measure | Pattern |
|---|---|
| `Revenue`, `Orders`, `Open orders`, `Cancelled orders`, `Late orders` | Additive `SUM(column)`; counts are 0/1 flags materialised upstream |
| `On-time %` | Ratio of two additive sums — no DAX, no measure references |
| `Average lead time` | `SUM(sum_x) / SUM(count_x)`; the sum/count pair is upstream |
| `Revenue YTD` | The flag (`ytd_flag`) is an upstream column; the proxy only filters on it |
| `Median lead time` | Non-additive metric served from `mart_lead_time_median_month` at its declared grain (month × product) |
| `Cumulative revenue CY` / `CY-1` | Window-function snapshots from `mart_cumulative_month` |

## The honest trade-offs

- **Grain pinning.** The median mart is valid at month × product. A yearly
  "median of monthly medians" is not a median — if the business wants yearly
  medians, materialise a yearly mart (or accept a fact scan). Cumulative
  measures use `MAX` over the period, which *is* correct at coarser grains
  because cumulative values are monotonic within a year.
- **Dimension scope.** The cumulative mart has no product, so pivoting it by
  `Category`/`Product` is ignored (SSAS-compatible). Add the dimension to the
  mart upstream if the business needs it.
- **Conformed join columns.** Facts that share a dimension use the same join
  column name (`order_date_key`, `product_key`), so one relationship per
  dimension serves them all.

## What not to do

If a metric needs logic that is not plain SQL over these tables — DAX, a
measure-of-measure, a time-intelligence function, a cumulative calculation at
an arbitrary grain — add it **upstream** as a column or a mart. `qualify
--strict` exists to keep it that way.
