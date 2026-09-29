MODEL (
  name upstream_marts.dim_date,
  kind FULL,
  grain (date_key),
  columns (
    date_key INT,
    full_date DATE,
    year INT,
    quarter INT,
    month INT,
    month_name TEXT,
    week INT,
    ytd_flag BOOLEAN,
    qtd_flag BOOLEAN,
    mtd_flag BOOLEAN,
    prior_year_ytd_flag BOOLEAN,
    current_year_flag BOOLEAN
  ),
  audits (
    not_null(columns := (date_key, full_date, year, quarter, month, month_name, week,
                         ytd_flag, qtd_flag, mtd_flag, prior_year_ytd_flag, current_year_flag)),
    unique_combination_of_columns(columns := (date_key))
  )
);

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
