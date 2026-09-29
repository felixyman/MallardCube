METRIC (
  name revenue,
  description 'Total revenue',
  expression SUM(upstream_marts.fact_orders.revenue)
);
