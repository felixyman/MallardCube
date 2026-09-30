METRIC (
  name revenue_per_unit,
  description 'Revenue per unit (ratio of sums)',
  expression SUM(main.fact_sales.net_revenue) / NULLIF(SUM(main.fact_sales.l_quantity), 0)
);
