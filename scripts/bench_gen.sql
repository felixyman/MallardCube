-- Large benchmark database matching projects/project3 schema.
-- sales_fact: 5M rows; date_dim: 2020-2030 calendar. Edit the row count in
-- `FROM range(5000000)` to scale the benchmark up or down.

CREATE TABLE IF NOT EXISTS date_dim AS
WITH RECURSIVE dates(d) AS (
    SELECT DATE '2020-01-01'
    UNION ALL
    SELECT d + 1 FROM dates WHERE d < DATE '2030-12-31'
)
SELECT
    strftime(d, '%Y%m%d')::INTEGER AS date_key,
    d::DATE AS full_date,
    strftime(d, '%Y')::INTEGER AS year,
    CEIL(strftime(d, '%m')::INTEGER / 3.0)::INTEGER AS quarter,
    strftime(d, '%m')::INTEGER AS month,
    d <= CURRENT_DATE AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y') AS ytd_flag,
    strftime(d, '%Y') = (strftime(CURRENT_DATE, '%Y')::INTEGER - 1)::TEXT
        AND strftime(d, '%j')::INTEGER <= strftime(CURRENT_DATE, '%j')::INTEGER AS prior_year_ytd_flag,
    strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y') AS current_year_flag,
    d <= CURRENT_DATE
        AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
        AND CEIL(strftime(d, '%m')::INTEGER / 3.0) = CEIL(strftime(CURRENT_DATE, '%m')::INTEGER / 3.0) AS qtd_flag,
    d <= CURRENT_DATE
        AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
        AND strftime(d, '%m') = strftime(CURRENT_DATE, '%m') AS mtd_flag
FROM dates;

CREATE TABLE IF NOT EXISTS sales_fact AS
SELECT
    list_value('Electronics','Clothing','Food','Furniture','Sports','Books','Toys','Automotive','Health','Music','Garden','Office','Pet Supplies','Jewelry','Home','Baby','Tools','Beauty','Shoes','Outdoors')[(abs(hash(i::VARCHAR)) % 20 + 1)::INTEGER] AS category,
    list_value('North','South','East','West','Central','Northeast','Southeast','Northwest')[(abs(hash(i::VARCHAR || 't')) % 8 + 1)::INTEGER] AS territory,
    list_value('Online','Retail','Wholesale','Direct')[(abs(hash(i::VARCHAR || 'c')) % 4 + 1)::INTEGER] AS channel,
    list_value('Consumer','Business','Government','Education','Non-Profit')[(abs(hash(i::VARCHAR || 's')) % 5 + 1)::INTEGER] AS segment,
    (abs(hash(i::VARCHAR || 'r')) % 50000 + 1000)::DOUBLE AS revenue,
    (abs(hash(i::VARCHAR || 'u')) % 500)::DOUBLE AS units,
    strftime(DATE '2020-01-01' + (abs(hash(i::VARCHAR || 'd')) % 4018)::INTEGER, '%Y%m%d')::INTEGER AS date_key
FROM range(5000000) t(i);
