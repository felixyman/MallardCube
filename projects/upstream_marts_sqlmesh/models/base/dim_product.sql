MODEL (
  name upstream_marts.dim_product,
  kind FULL,
  grain (product_key),
  columns (
    product_key INT,
    product_name TEXT,
    category TEXT
  ),
  audits (
    not_null(columns := (product_key, product_name, category)),
    unique_combination_of_columns(columns := (product_key))
  )
);

SELECT row_number() OVER () AS product_key,
       category || ' ' || lpad(CAST(n AS VARCHAR), 2, '0') AS product_name,
       category
FROM (SELECT unnest(['Hardware', 'Software', 'Services']) AS category),
     (SELECT unnest([1, 2, 3, 4]) AS n);
