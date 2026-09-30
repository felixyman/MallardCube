# tpch_sqlmesh

The contract generator (plan 057-A) run against a real schema: TPC-H, generated
locally by DuckDB's `tpch` extension. The raw tables are EXTERNAL SQLMesh
sources; the served model is a conformed line-item fact joined to customer,
part, supplier, nation and order-date dimensions, plus a monthly revenue mart
for the cumulative value.

## Build and run

```bash
# One-time: the DuckDB tpch extension fetches itself on INSTALL.
duckdb data/tpch.duckdb -c "INSTALL tpch; LOAD tpch; CREATE SCHEMA raw; CALL dbgen(sf=0.1, schema='raw')"

# Materialise with the uv-managed SQLMesh (oracles/sqlmesh).
cd projects/tpch_sqlmesh
uv run --project ../../oracles/sqlmesh sqlmesh plan --auto-apply

# Generate, project and qualify the contract.
../../target/release/mallard contract generate --from sqlmesh . --out contract.yaml
../../target/release/mallard qualify proxy-config.yaml --contract contract.yaml
```

`data/` is generated and not checked in; `contract.yaml` and
`contract.overlay.yaml` are. At SF=0.1 the fact has 600,572 rows (150,000
orders, 15,000 customers, 20,000 parts, 1,000 suppliers, 25 nations); SF=1 is
the same shape at ten times the size.

## Measured (2026-09-30, SF=0.1)

- The generator produced the contract on the first run: 2 grain tables, 5
  dimensions, 5 relationships, 9 measures (five sums, two ratios of sums, one
  `COUNT(*)` via the overlay's declared source, one `COUNT(DISTINCT …)`, and a
  `time_window` YTD measure).
- `mallard qualify proxy-config.yaml --contract contract.yaml` is **READY** in
  ~2.3 s at 600,572 fact rows: grain uniqueness, `valid_grain` validity, SQL
  bindings, column existence and the projection correspondence all pass.
- The served pivot matches direct SQL exactly: revenue by year
  (1992 `3,124,626,848.72` … 1998 `1,859,264,398.88`), order counts and the
  average discount (0.0499–0.0501); `Revenue YTD` (`1,859,264,398.88`) equals
  the `ytd_flag` slice.
- Two real-world findings the synthetic fixtures could not show: SQLMesh
  `EXTERNAL` models require a query body (a self-named
  `SELECT * FROM raw.<table>` works), and MallardCube serves bare table names,
  so the materialised models must land in the connection's default schema
  (`main`), not a named one.

The checker also confirmed the wiring end to end: the checked-in
`contract.yaml` is byte-stable (`--check`), and `proxy-config.yaml` is the
projection of it.

## SF=1 (2026-09-30)

dbgen 12 s, SQLMesh materialise 9 s for 6,001,215 fact rows.

- `contract generate --check` still matches byte for byte: the contract is
  data-independent.
- `qualify --contract` is READY in **11.2 s** (SF=0.1: 2.3 s).
- The pivot matches the SQL oracle again: 1992 revenue `33,009,798,732.77`,
  1998 `19,433,822,171.60`; `Revenue YTD` is the 1998 slice and zero for the
  earlier years.
- **Recorded gap** (visible only with real data): a windowed measure over a
  period with no matching rows returns `0`, where the reference returns an
  empty cell and Excel shows blank — the value path carries `f64`, not
  `Option<f64>`. Recorded in plan 057 for a slice of its own, with a reference
  measurement of the empty-cell encoding.
