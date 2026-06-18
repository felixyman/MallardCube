-- Bootstrap script for DW_FYS_F_UNDERSÖKNING
-- Run against DuckDB to create a runnable database.
--   duckdb data/f_undersokning.db < bootstrap.sql

.read schema.sql
.read seed_date_dim.sql
