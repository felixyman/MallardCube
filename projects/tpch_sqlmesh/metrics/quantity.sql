METRIC (
  name quantity,
  description 'Total quantity',
  expression SUM(main.fact_sales.l_quantity)
);
