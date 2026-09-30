MODEL (
  name main.dim_supplier,
  kind FULL,
  grain (s_suppkey),
  columns (
    s_suppkey BIGINT,
    s_name TEXT,
    s_nationkey BIGINT
  ),
  audits (
    not_null(columns := (s_suppkey, s_name)),
    unique_combination_of_columns(columns := (s_suppkey))
  )
);

SELECT s_suppkey, s_name, s_nationkey FROM raw.supplier;
