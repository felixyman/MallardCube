METRIC (
  name revenue_ytd,
  description 'Revenue year-to-date',
  expression SUM(main.fact_sales.net_revenue)
);
