MODEL (
  name main.dim_nation,
  kind FULL,
  grain (n_nationkey),
  columns (
    n_nationkey BIGINT,
    n_name TEXT
  ),
  audits (
    not_null(columns := (n_nationkey, n_name)),
    unique_combination_of_columns(columns := (n_nationkey))
  )
);

SELECT n_nationkey, n_name FROM raw.nation;
