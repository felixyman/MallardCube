MODEL (
  name upstream_marts.mart_lead_time_median_month,
  kind FULL,
  grain (year, month, product_key),
  columns (
    year INT,
    month INT,
    order_date_key INT,
    product_key INT,
    median_lead_time DOUBLE
  ),
  audits (
    not_null(columns := (year, month, order_date_key, product_key, median_lead_time)),
    unique_combination_of_columns(columns := (year, month, product_key)),
    relationships(column := order_date_key,
                 reference := upstream_marts.dim_date,
                 reference_column := date_key),
    relationships(column := product_key,
                 reference := upstream_marts.dim_product,
                 reference_column := product_key)
  )
);

SELECT dt.year,
       dt.month,
       (dt.year * 10000 + dt.month * 100 + 1) AS order_date_key,
       f.product_key,
       MEDIAN(f.lead_time_days) AS median_lead_time
FROM upstream_marts.fact_orders f
JOIN upstream_marts.dim_date dt ON f.order_date_key = dt.date_key
WHERE f.lead_time_days IS NOT NULL
GROUP BY 1, 2, 3, 4;
