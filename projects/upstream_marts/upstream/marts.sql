-- Per-grain aggregate marts (plan 044).
--
-- Non-additive metrics (medians) and cumulative metrics cannot be re-aggregated
-- from the fact at query time without lying about the grain, so they are
-- materialised here at a declared grain. The proxy exposes them as measures
-- over these tables and never re-computes them.

-- Median lead time per month x product. Valid at this grain only: a yearly
-- "median of monthly medians" is not a median.
CREATE TABLE mart_lead_time_median_month AS
SELECT dt.year,
       dt.month,
       (dt.year * 10000 + dt.month * 100 + 1) AS order_date_key, -- month-first day
       f.product_key,
       MEDIAN(f.lead_time_days) AS median_lead_time
FROM fact_orders f
JOIN dim_date dt ON f.order_date_key = dt.date_key
WHERE f.lead_time_days IS NOT NULL
GROUP BY 1, 2, 3, 4;

-- Cumulative (year-to-date) revenue per month, plus the same month last year.
-- Cumulative values are monotonic inside a year, so taking the last month of a
-- coarser period (MAX over the period) is the correct period-end value.
CREATE TABLE mart_cumulative_month AS
WITH monthly AS (
    SELECT dt.year,
           dt.month,
           (dt.year * 10000 + dt.month * 100 + 1) AS order_date_key,
           SUM(f.revenue) AS month_revenue
    FROM fact_orders f
    JOIN dim_date dt ON f.order_date_key = dt.date_key
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
