METRIC (
  name cumulative_revenue_cy,
  description 'Cumulative revenue for the current year, period-end value',
  expression MAX(upstream_marts.mart_cumulative_month.cumulative_revenue_cy)
);
