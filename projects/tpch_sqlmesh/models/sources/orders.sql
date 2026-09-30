MODEL (
  name raw.orders,
  kind EXTERNAL,
  columns (
    o_orderkey BIGINT,
    o_custkey BIGINT,
    o_orderstatus TEXT,
    o_totalprice DECIMAL(15,2),
    o_orderdate DATE,
    o_orderpriority TEXT,
    o_clerk TEXT,
    o_shippriority INTEGER,
    o_comment TEXT
  )
);
SELECT * FROM raw.orders;
