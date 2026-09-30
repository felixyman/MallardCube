METRIC (
  name cumulative_revenue,
  description 'Cumulative revenue for the year',
  expression MAX(main.mart_monthly_revenue.cumulative_revenue_cy)
);
