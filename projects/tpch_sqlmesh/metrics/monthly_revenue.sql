METRIC (
  name monthly_revenue,
  description 'Revenue for the month',
  expression MAX(main.mart_monthly_revenue.month_revenue)
);
