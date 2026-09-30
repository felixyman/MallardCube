MODEL (
  name raw.nation,
  kind EXTERNAL,
  columns (
    n_nationkey BIGINT,
    n_name TEXT,
    n_regionkey BIGINT,
    n_comment TEXT
  )
);
SELECT * FROM raw.nation;
