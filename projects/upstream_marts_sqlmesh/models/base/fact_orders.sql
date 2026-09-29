MODEL (
  name upstream_marts.fact_orders,
  kind FULL,
  grain (order_id),
  columns (
    order_id INT,
    order_date_key INT,
    customer_key INT,
    product_key INT,
    status TEXT,
    is_order INT,
    is_open INT,
    is_closed INT,
    is_cancelled INT,
    is_late INT,
    is_within_sla INT,
    lead_time_days INT,
    sum_lead_time DOUBLE,
    count_lead_time INT,
    quantity INT,
    revenue DOUBLE
  ),
  audits (
    not_null(columns := (order_id, order_date_key, customer_key, product_key, status,
                         is_order, is_open, is_closed, is_cancelled, is_late, is_within_sla,
                         sum_lead_time, count_lead_time, quantity, revenue)),
    unique_combination_of_columns(columns := (order_id)),
    relationships(column := order_date_key,
                 reference := upstream_marts.dim_date,
                 reference_column := date_key),
    relationships(column := customer_key,
                 reference := upstream_marts.dim_customer,
                 reference_column := customer_key),
    relationships(column := product_key,
                 reference := upstream_marts.dim_product,
                 reference_column := product_key)
  )
);

WITH RECURSIVE days(day) AS (
    SELECT DATE '2024-01-01'
    UNION ALL
    SELECT day + 1 FROM days WHERE day < CURRENT_DATE
),
dates AS (
    SELECT CAST(strftime(day, '%Y%m%d') AS INTEGER) AS date_key,
           row_number() OVER (ORDER BY day) - 1 AS dn,
           COUNT(*) OVER () AS total
    FROM days
),
customers AS (
    SELECT customer_key, row_number() OVER (ORDER BY customer_key) - 1 AS c FROM upstream_marts.dim_customer
),
products AS (
    SELECT product_key, row_number() OVER (ORDER BY product_key) - 1 AS p FROM upstream_marts.dim_product
),
base AS (
    SELECT i,
           dt.date_key,
           cu.customer_key,
           pr.product_key,
           CASE
               WHEN i % 7 = 0 THEN 'Cancelled'
               WHEN i % 3 = 0 THEN 'Open'
               ELSE 'Closed'
           END AS status,
           CASE
               WHEN i % 7 != 0 AND i % 3 != 0 THEN 3 + (i * 5) % 30
               ELSE NULL
           END AS lead_time_days,
           1 + i % 5 AS quantity,
           round(50 + (i * 37) % 950 + (i % 7) * 3.5, 2) AS revenue
    FROM range(1, 1501) AS t(i)
    JOIN dates dt ON dt.dn = i % (SELECT total FROM dates LIMIT 1)
    JOIN customers cu ON cu.c = i % 24
    JOIN products pr ON pr.p = (i * 7) % 12
)
SELECT i AS order_id,
       date_key AS order_date_key,
       customer_key,
       product_key,
       status,
       1 AS is_order,
       CASE WHEN status = 'Open' THEN 1 ELSE 0 END AS is_open,
       CASE WHEN status = 'Closed' THEN 1 ELSE 0 END AS is_closed,
       CASE WHEN status = 'Cancelled' THEN 1 ELSE 0 END AS is_cancelled,
       CASE WHEN lead_time_days > 14 THEN 1 ELSE 0 END AS is_late,
       CASE WHEN lead_time_days IS NOT NULL AND lead_time_days <= 14 THEN 1 ELSE 0 END
           AS is_within_sla,
       lead_time_days,
       COALESCE(lead_time_days, 0) AS sum_lead_time,
       CASE WHEN lead_time_days IS NULL THEN 0 ELSE 1 END AS count_lead_time,
       quantity,
       revenue
FROM base;
