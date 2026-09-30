METRIC (
  name order_count,
  description 'Distinct orders',
  expression COUNT(DISTINCT main.fact_sales.l_orderkey)
);
