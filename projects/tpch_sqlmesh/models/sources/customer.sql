MODEL (
  name raw.customer,
  kind EXTERNAL,
  columns (
    c_custkey BIGINT,
    c_name TEXT,
    c_address TEXT,
    c_nationkey BIGINT,
    c_phone TEXT,
    c_acctbal DECIMAL(15,2),
    c_mktsegment TEXT,
    c_comment TEXT
  )
);
SELECT * FROM raw.customer;
