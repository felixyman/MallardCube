METRIC (
  name avg_lead_time,
  description 'Average lead time in days (ratio of sums)',
  expression SUM(upstream_marts.fact_orders.sum_lead_time)
             / NULLIF(SUM(upstream_marts.fact_orders.count_lead_time), 0)
);
