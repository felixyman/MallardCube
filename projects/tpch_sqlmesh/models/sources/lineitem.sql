MODEL (
  name raw.lineitem,
  kind EXTERNAL,
  columns (
    l_orderkey BIGINT,
    l_partkey BIGINT,
    l_suppkey BIGINT,
    l_linenumber INTEGER,
    l_quantity DECIMAL(15,2),
    l_extendedprice DECIMAL(15,2),
    l_discount DECIMAL(15,2),
    l_tax DECIMAL(15,2),
    l_returnflag TEXT,
    l_linestatus TEXT,
    l_shipdate DATE,
    l_commitdate DATE,
    l_receiptdate DATE,
    l_shipinstruct TEXT,
    l_shipmode TEXT,
    l_comment TEXT
  )
);
SELECT * FROM raw.lineitem;
