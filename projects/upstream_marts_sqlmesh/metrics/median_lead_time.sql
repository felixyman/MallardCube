METRIC (
  name median_lead_time,
  description 'Median lead time in days, valid at month x product only',
  expression MAX(upstream_marts.mart_lead_time_median_month.median_lead_time)
);
