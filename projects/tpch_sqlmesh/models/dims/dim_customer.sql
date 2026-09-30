MODEL (
  name main.dim_customer,
  kind FULL,
  grain (c_custkey),
  columns (
    c_custkey BIGINT,
    c_name TEXT,
    c_mktsegment TEXT
  ),
  audits (
    not_null(columns := (c_custkey, c_name, c_mktsegment)),
    unique_combination_of_columns(columns := (c_custkey))
  )
);

SELECT c_custkey, c_name, c_mktsegment FROM raw.customer;
