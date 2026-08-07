-- Bootstrap script for SALES
-- Run against DuckDB to create a runnable database.
--   duckdb data/sales.db < bootstrap.sql

.read schema.sql
.read seed_date_dim.sql
