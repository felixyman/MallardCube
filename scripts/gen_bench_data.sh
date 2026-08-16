#!/usr/bin/env bash
# Generate the MallardCube benchmark database as Parquet + a tiny DuckDB file
# whose tables are VIEWs over the Parquet. DuckDB reads Parquet column-by-column,
# so this exercises the realistic Phase-3 storage path with no proxy changes.
#
# Usage:
#   ROWS=500000000 BENCH_DIR=/path bash scripts/gen_bench_data.sh
#
# ROWS defaults to 5,000,000 (matches the original baseline).
set -euo pipefail

ROWS="${ROWS:-5000000}"
BENCH_DIR="${BENCH_DIR:-/tmp/mallardcube-bench}"
mkdir -p "$BENCH_DIR"

FACT="$BENCH_DIR/sales_fact.parquet"
DIM="$BENCH_DIR/date_dim.parquet"
DB="$BENCH_DIR/sales_large.duckdb"

command -v duckdb >/dev/null || { echo "duckdb CLI required" >&2; exit 1; }

echo "==> date_dim.parquet (2020-2030 calendar)"
duckdb :memory: "COPY (
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
    d <= CURRENT_DATE AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
      AND CEIL(strftime(d, '%m')::INTEGER / 3.0) = CEIL(strftime(CURRENT_DATE, '%m')::INTEGER / 3.0) AS qtd_flag,
    d <= CURRENT_DATE AND strftime(d, '%Y') = strftime(CURRENT_DATE, '%Y')
      AND strftime(d, '%m') = strftime(CURRENT_DATE, '%m') AS mtd_flag
  FROM dates
) TO '$DIM' (FORMAT PARQUET);"

echo "==> sales_fact.parquet ($ROWS rows)"
duckdb :memory: "COPY (
  SELECT
    list_value('Electronics','Clothing','Food','Furniture','Sports','Books','Toys','Automotive','Health','Music','Garden','Office','Pet Supplies','Jewelry','Home','Baby','Tools','Beauty','Shoes','Outdoors')[(i % 20 + 1)::INTEGER] AS category,
    list_value('North','South','East','West','Central','Northeast','Southeast','Northwest')[(i % 8 + 1)::INTEGER] AS territory,
    list_value('Online','Retail','Wholesale','Direct')[(i % 4 + 1)::INTEGER] AS channel,
    list_value('Consumer','Business','Government','Education','Non-Profit')[(i % 5 + 1)::INTEGER] AS segment,
    ((i % 50000) + 1000)::DOUBLE AS revenue,
    (i % 500)::DOUBLE AS units,
    strftime(DATE '2020-01-01' + (i % 4018)::INTEGER, '%Y%m%d')::INTEGER AS date_key
  FROM range($ROWS) t(i)
) TO '$FACT' (FORMAT PARQUET);"

echo "==> $DB (views over the Parquet files)"
rm -f "$DB"
duckdb "$DB" "CREATE VIEW sales_fact AS SELECT * FROM read_parquet('$FACT');
              CREATE VIEW date_dim AS SELECT * FROM read_parquet('$DIM');"

echo "==> done:"
duckdb "$DB" -c "SELECT 'sales_fact' t, COUNT(*) n, CAST(SUM(revenue) AS BIGINT) revenue FROM sales_fact UNION ALL SELECT 'date_dim', COUNT(*), 0 FROM date_dim;"
ls -lh "$FACT" "$DIM" "$DB"
