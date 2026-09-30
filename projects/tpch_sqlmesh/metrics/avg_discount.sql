METRIC (
  name avg_discount,
  description 'Average discount (ratio of sums)',
  expression SUM(main.fact_sales.l_extendedprice * main.fact_sales.l_discount)
  / NULLIF(SUM(main.fact_sales.l_extendedprice), 0)
);
