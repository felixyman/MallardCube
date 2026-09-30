MODEL (
  name main.mart_monthly_revenue,
  kind FULL,
  grain (year, month),
  columns (
    year INTEGER,
    month INTEGER,
    month_revenue DECIMAL(18,2),
    cumulative_revenue_cy DECIMAL(18,2)
  ),
  audits (
    not_null(columns := (year, month, month_revenue, cumulative_revenue_cy)),
    unique_combination_of_columns(columns := (year, month))
  )
);

-- A per-grain mart: cumulative revenue is a period-end value, valid at the
-- month grain only.
WITH monthly AS (
    SELECT year(o.o_orderdate) AS year,
           month(o.o_orderdate) AS month,
           SUM(l.l_extendedprice * (1 - l.l_discount)) AS month_revenue
    FROM raw.lineitem AS l
    JOIN raw.orders AS o ON l.l_orderkey = o.o_orderkey
    GROUP BY 1, 2
)
SELECT year,
       month,
       month_revenue,
       SUM(month_revenue) OVER (PARTITION BY year ORDER BY month) AS cumulative_revenue_cy
FROM monthly;
