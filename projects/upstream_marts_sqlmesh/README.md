# upstream_marts_sqlmesh

The same model surface as `projects/upstream_marts` — dimensions, a conformed
fact and per-grain aggregate marts — expressed as a real SQLMesh project. It is
the oracle for the contract generators (plan 057-A, section D): a generator
parses these files, and this project is what a real SQLMesh stack would run.

## Run it

The environment lives in `oracles/sqlmesh` (uv-managed; not used by the product
or CI):

```bash
cd projects/upstream_marts_sqlmesh
uv run --project ../../oracles/sqlmesh sqlmesh plan --auto-apply
```

The DuckDB file lands in `data/` (not checked in). With
`physical_schema_mapping` the models materialise as views in
`upstream_marts.*` over the physical `<schema>__<model>__<hash>` tables, so the
same file is directly usable by the proxy.

## Measured against SQLMesh 0.236.2 (2026-09-29)

- Model files carry `grain (cols)`, `columns (…)`, `kind`, the SQL body, and
  audit calls with named arguments. Metrics live in `metrics/*.sql` as
  `METRIC (name, description, expression)`; the expression is SQL, not a
  declared aggregation.
- There is **no built-in `relationships` audit** (that name is dbt's); a real
  project declares a custom one, and the *call site* is what a generator can
  read: `relationships(column := …, reference := schema.model,
  reference_column := …)`.
- Audit queries are **not** rewritten to physical tables (model queries are),
  and the environment's logical views do not exist yet while a plan applies, so
  a cross-model FK check cannot run as a plain SQL audit.
  `audits/relationships.sql` therefore carries the declaration and is
  `skip true`; the check itself runs in `mallard qualify --contract` (orphans
  and fan-out against the materialised data).
- `physical_schema_override` is deprecated in favour of
  `physical_schema_mapping`.
- The built data matches `projects/upstream_marts`: 1,500 orders, revenue
  801,339.50, 264 median rows, 33 cumulative rows.
