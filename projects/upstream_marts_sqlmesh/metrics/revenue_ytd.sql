METRIC (
  name revenue_ytd,
  description 'Revenue year-to-date',
  expression SUM(upstream_marts.fact_orders.revenue)
);
