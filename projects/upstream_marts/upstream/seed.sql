-- Deterministic demo data for the upstream schema (plan 044).
-- No future dates: the calendar ends at CURRENT_DATE, and the flags are
-- computed against CURRENT_DATE so YTD/QTD/MTD are meaningful.

INSERT INTO dim_customer
SELECT row_number() OVER () AS customer_key,
       region || ' ' || segment || ' ' || lpad(CAST(n AS VARCHAR), 2, '0') AS customer_name,
       region,
       segment
FROM (SELECT unnest(['North', 'South', 'East', 'West']) AS region),
     (SELECT unnest(['Enterprise', 'SMB']) AS segment),
     (SELECT unnest([1, 2, 3]) AS n);

INSERT INTO dim_product
SELECT row_number() OVER () AS product_key,
       category || ' ' || lpad(CAST(n AS VARCHAR), 2, '0') AS product_name,
       category
FROM (SELECT unnest(['Hardware', 'Software', 'Services']) AS category),
     (SELECT unnest([1, 2, 3, 4]) AS n);

INSERT INTO dim_date
WITH RECURSIVE days(day) AS (
    SELECT DATE '2024-01-01'
    UNION ALL
    SELECT day + 1 FROM days WHERE day < CURRENT_DATE
)
SELECT CAST(strftime(day, '%Y%m%d') AS INTEGER) AS date_key,
       day AS full_date,
       year(day) AS year,
       quarter(day) AS quarter,
       month(day) AS month,
       strftime(day, '%B') AS month_name,
       week(day) AS week,
       year(day) = year(CURRENT_DATE) AND day <= CURRENT_DATE AS ytd_flag,
       year(day) = year(CURRENT_DATE)
           AND quarter(day) = quarter(CURRENT_DATE) AND day <= CURRENT_DATE AS qtd_flag,
       year(day) = year(CURRENT_DATE)
           AND month(day) = month(CURRENT_DATE) AND day <= CURRENT_DATE AS mtd_flag,
       year(day) = year(CURRENT_DATE) - 1 AS prior_year_ytd_flag,
       year(day) = year(CURRENT_DATE) AS current_year_flag
FROM days;

-- 1,500 orders spread deterministically over dates, customers, and products.
INSERT INTO fact_orders
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
    SELECT customer_key, row_number() OVER (ORDER BY customer_key) - 1 AS c FROM dim_customer
),
products AS (
    SELECT product_key, row_number() OVER (ORDER BY product_key) - 1 AS p FROM dim_product
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
