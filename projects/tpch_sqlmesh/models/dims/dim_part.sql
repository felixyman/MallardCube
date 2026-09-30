MODEL (
  name main.dim_part,
  kind FULL,
  grain (p_partkey),
  columns (
    p_partkey BIGINT,
    p_name TEXT,
    p_brand TEXT,
    p_type TEXT,
    p_size INTEGER,
    p_container TEXT
  ),
  audits (
    not_null(columns := (p_partkey, p_name, p_brand, p_type, p_size, p_container)),
    unique_combination_of_columns(columns := (p_partkey))
  )
);

SELECT p_partkey, p_name, p_brand, p_type, p_size, p_container FROM raw.part;
