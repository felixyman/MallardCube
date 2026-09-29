METRIC (
  name late_orders,
  description 'Late orders',
  expression SUM(upstream_marts.fact_orders.is_late)
);
