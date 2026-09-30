-- The custom FK audit (see projects/upstream_marts_sqlmesh/audits/): SQLMesh
-- has no built-in relationships audit, and a cross-model reference cannot run
-- as a plain SQL audit, so the declaration is carried and the FK check runs in
-- `mallard qualify --contract`.

AUDIT (
  name relationships,
  dialect duckdb,
  skip true,
);

SELECT child.*
FROM @this_model AS child
WHERE child.@column IS NOT NULL
  AND NOT EXISTS (
    SELECT 1
    FROM @reference AS parent
    WHERE parent.@reference_column = child.@column
  );
