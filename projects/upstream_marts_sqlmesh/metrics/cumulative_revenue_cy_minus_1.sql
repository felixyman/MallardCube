METRIC (
  name cumulative_revenue_cy_minus_1,
  description 'Cumulative revenue for the same month last year, period-end value',
  expression MAX(upstream_marts.mart_cumulative_month.cumulative_revenue_cy_minus_1)
);
