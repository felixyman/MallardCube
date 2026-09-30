MODEL (
  name raw.supplier,
  kind EXTERNAL,
  columns (
    s_suppkey BIGINT,
    s_name TEXT,
    s_address TEXT,
    s_nationkey BIGINT,
    s_phone TEXT,
    s_acctbal DECIMAL(15,2),
    s_comment TEXT
  )
);
SELECT * FROM raw.supplier;
