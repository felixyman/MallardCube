-- Date dimension calendar table: 2020-01-01 through 2030-12-31
-- DuckDB SQL. Run via Backend::execute_ddl().

CREATE TABLE IF NOT EXISTS date_dim AS
WITH RECURSIVE dates(d) AS (
    SELECT '2020-01-01'::DATE
    UNION ALL
    SELECT d + 1 FROM dates WHERE d < '2030-12-31'::DATE
)
SELECT
    strftime(d, '%Y%m%d')::INTEGER AS date_key,
    d::DATE AS full_date,
    strftime(d, '%Y')::INTEGER AS year,
    CEIL(strftime(d, '%m')::INTEGER / 3.0)::INTEGER AS quarter,
    strftime(d, '%m')::INTEGER AS month,
    -- Year To Date: dates in the current year up to today
    d <= CURRENT_DATE AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y') AS ytd_flag,
    -- Prior Year YTD: same day-of-year range in the previous year
    strftime(d, '%Y') = (strftime(CURRENT_DATE, '%Y')::INTEGER - 1)::TEXT
        AND strftime(d, '%j')::INTEGER <= strftime(CURRENT_DATE, '%j')::INTEGER AS prior_year_ytd_flag,
    -- Current Year: all dates in the current year (not just YTD)
    strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y') AS current_year_flag,
    -- Quarter To Date: dates in the current quarter up to today
    d <= CURRENT_DATE
        AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
        AND CEIL(strftime(d, '%m')::INTEGER / 3.0) = CEIL(strftime(CURRENT_DATE, '%m')::INTEGER / 3.0) AS qtd_flag,
    -- Month To Date: dates in the current month up to today
    d <= CURRENT_DATE
        AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
        AND strftime(d, '%m') = strftime(CURRENT_DATE, '%m') AS mtd_flag
FROM dates;
