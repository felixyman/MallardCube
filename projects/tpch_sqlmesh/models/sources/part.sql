MODEL (
  name raw.part,
  kind EXTERNAL,
  columns (
    p_partkey BIGINT,
    p_name TEXT,
    p_mfgr TEXT,
    p_brand TEXT,
    p_type TEXT,
    p_size INTEGER,
    p_container TEXT,
    p_retailprice DECIMAL(15,2),
    p_comment TEXT
  )
);
SELECT * FROM raw.part;
