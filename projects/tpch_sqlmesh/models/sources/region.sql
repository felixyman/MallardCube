MODEL (
  name raw.region,
  kind EXTERNAL,
  columns (
    r_regionkey BIGINT,
    r_name TEXT,
    r_comment TEXT
  )
);
SELECT * FROM raw.region;
