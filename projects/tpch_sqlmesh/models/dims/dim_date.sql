MODEL (
  name main.dim_date,
  kind FULL,
  grain (date_key),
  columns (
    date_key INTEGER,
    full_date DATE,
    year INTEGER,
    quarter INTEGER,
    month INTEGER,
    month_name TEXT,
    ytd_flag BOOLEAN,
    qtd_flag BOOLEAN,
    mtd_flag BOOLEAN,
    prior_year_ytd_flag BOOLEAN,
    current_year_flag BOOLEAN
  ),
  audits (
    not_null(columns := (date_key, full_date, year, quarter, month, month_name,
                         ytd_flag, qtd_flag, mtd_flag, prior_year_ytd_flag, current_year_flag)),
    unique_combination_of_columns(columns := (date_key))
  )
);

-- Order dates only, with flags relative to the dataset's own "today" (the
-- latest order date): TPC-H data is historical, so CURRENT_DATE would make
-- every flag false.
WITH reference AS (
    SELECT MAX(o_orderdate) AS today FROM raw.orders
)
SELECT CAST(strftime(d, '%Y%m%d') AS INTEGER) AS date_key,
       d AS full_date,
       year(d) AS year,
       quarter(d) AS quarter,
       month(d) AS month,
       strftime(d, '%B') AS month_name,
       year(d) = year(today) AND d <= today AS ytd_flag,
       year(d) = year(today) AND quarter(d) = quarter(today) AND d <= today AS qtd_flag,
       year(d) = year(today) AND month(d) = month(today) AND d <= today AS mtd_flag,
       year(d) = year(today) - 1 AS prior_year_ytd_flag,
       year(d) = year(today) AS current_year_flag
FROM (SELECT DISTINCT o_orderdate AS d FROM raw.orders)
CROSS JOIN reference;
