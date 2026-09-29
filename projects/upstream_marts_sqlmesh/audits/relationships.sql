-- A custom FK audit: SQLMesh 0.236 has no built-in relationships audit, so a
-- real project declares one. The call site in the model files is what the
-- contract generator reads:
--
--   relationships(column := product_key,
--                 reference := upstream_marts.dim_product,
--                 reference_column := product_key)
--
-- Measured 2026-09-29: audit queries are *not* rewritten to physical tables
-- (model queries are), and the environment's logical views do not exist yet
-- while a plan applies, so a cross-model reference cannot run as a plain SQL
-- audit. The declaration is therefore carried and skipped here; the FK check
-- itself runs in MallardCube's qualifier (`mallard qualify --contract`), which
-- reports orphans and fan-out against the materialised data.

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
