MODEL (
  name raw.partsupp,
  kind EXTERNAL,
  columns (
    ps_partkey BIGINT,
    ps_suppkey BIGINT,
    ps_availqty INTEGER,
    ps_supplycost DECIMAL(15,2),
    ps_comment TEXT
  )
);
SELECT * FROM raw.partsupp;
