-- Bootstrap script for SALES
-- Run against DuckDB to create a runnable database.
--   duckdb sales.db < bootstrap.sql

.read schema.sql
.read load_dummy_data.sql

-- For real data, replace the line above with:
-- .read load_data.sql
