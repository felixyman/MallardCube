METRIC (
  name on_time_pct,
  description 'On-time percentage (ratio of sums)',
  expression SUM(upstream_marts.fact_orders.is_within_sla) * 100.0
             / NULLIF(SUM(upstream_marts.fact_orders.is_order), 0)
);
