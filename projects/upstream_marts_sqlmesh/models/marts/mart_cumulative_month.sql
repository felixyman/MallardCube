MODEL (
  name upstream_marts.mart_cumulative_month,
  kind FULL,
  grain (year, month),
  columns (
    year INT,
    month INT,
    order_date_key INT,
    month_revenue DOUBLE,
    cumulative_revenue_cy DOUBLE,
    cumulative_revenue_cy_minus_1 DOUBLE
  ),
  audits (
    not_null(columns := (year, month, order_date_key, month_revenue, cumulative_revenue_cy)),
    unique_combination_of_columns(columns := (year, month)),
    relationships(column := order_date_key,
                 reference := upstream_marts.dim_date,
                 reference_column := date_key)
  )
);

WITH monthly AS (
    SELECT dt.year,
           dt.month,
           (dt.year * 10000 + dt.month * 100 + 1) AS order_date_key,
           SUM(f.revenue) AS month_revenue
    FROM upstream_marts.fact_orders f
    JOIN upstream_marts.dim_date dt ON f.order_date_key = dt.date_key
    GROUP BY 1, 2, 3
),
cumulative AS (
    SELECT year,
           month,
           order_date_key,
           month_revenue,
           SUM(month_revenue) OVER (PARTITION BY year ORDER BY month) AS cumulative_revenue_cy
    FROM monthly
)
SELECT cy.year,
       cy.month,
       cy.order_date_key,
       cy.month_revenue,
       cy.cumulative_revenue_cy,
       py.cumulative_revenue_cy AS cumulative_revenue_cy_minus_1
FROM cumulative cy
LEFT JOIN cumulative py ON py.year = cy.year - 1 AND py.month = cy.month;
