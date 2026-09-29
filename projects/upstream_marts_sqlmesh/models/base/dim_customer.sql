MODEL (
  name upstream_marts.dim_customer,
  kind FULL,
  grain (customer_key),
  columns (
    customer_key INT,
    customer_name TEXT,
    region TEXT,
    segment TEXT
  ),
  audits (
    not_null(columns := (customer_key, customer_name, region, segment)),
    unique_combination_of_columns(columns := (customer_key))
  )
);

SELECT row_number() OVER () AS customer_key,
       region || ' ' || segment || ' ' || lpad(CAST(n AS VARCHAR), 2, '0') AS customer_name,
       region,
       segment
FROM (SELECT unnest(['North', 'South', 'East', 'West']) AS region),
     (SELECT unnest(['Enterprise', 'SMB']) AS segment),
     (SELECT unnest([1, 2, 3]) AS n);
