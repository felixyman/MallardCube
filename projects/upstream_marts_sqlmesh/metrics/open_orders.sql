METRIC (
  name open_orders,
  description 'Open orders',
  expression SUM(upstream_marts.fact_orders.is_open)
);
