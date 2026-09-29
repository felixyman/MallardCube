METRIC (
  name cancelled_orders,
  description 'Cancelled orders',
  expression SUM(upstream_marts.fact_orders.is_cancelled)
);
