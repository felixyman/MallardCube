METRIC (
  name revenue,
  description 'Total revenue',
  expression SUM(main.fact_sales.net_revenue)
);
