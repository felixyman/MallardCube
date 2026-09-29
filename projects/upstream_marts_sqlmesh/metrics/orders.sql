METRIC (
  name orders,
  description 'Order count',
  expression SUM(upstream_marts.fact_orders.is_order)
);
