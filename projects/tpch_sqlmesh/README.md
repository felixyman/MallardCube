# tpch_sqlmesh

The contract generator (plan 057-A) run against a real schema: TPC-H, generated
locally by DuckDB's `tpch` extension. The raw tables are EXTERNAL SQLMesh
sources; the served model is a conformed line-item fact joined to customer,
part, supplier, nation and order-date dimensions, plus a monthly revenue mart
for the cumulative value.

## Build and run

```bash
# From the project directory (paths are relative to it).
cd projects/tpch_sqlmesh

# One-time: the DuckDB tpch extension fetches itself on INSTALL.
duckdb data/tpch.duckdb -c "INSTALL tpch; LOAD tpch; CREATE SCHEMA raw; CALL dbgen(sf=0.1, schema='raw')"

# Materialise with the uv-managed SQLMesh (oracles/sqlmesh).
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
  average discount (0.0499–0.0501). `Revenue YTD` is `1,859,264,398.88`,
  which equals the 1998 total because the dataset's last order date is
  1998-08-02 and `ytd_flag` therefore covers every 1998 row.
- `oracles.json` records the semantic expectations (totals, one Nation
  slice, both ratios, the monthly and cumulative maxima, the YTD slice), so
  `qualify` re-checks them against the proxy's SQL instead of trusting prose.
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
- The pivot matches the SQL oracle again: 1992 revenue `33,009,798,587.52`
(raw tables), 1998 `19,433,822,086.61`; `Revenue YTD` is the 1998 slice and
  zero for the earlier years.
- **Do not expect a 10:1 revenue ratio between scale factors.** TPC-H part
  prices grow with the part-key range (average `1409.5` over the first
  20,000 parts, `1499.5` over all 200,000), so per-line revenue rises with
  SF: totals are `20.54e9` (SF=0.1) vs `218.10e9` (SF=1), a factor of 10.62,
  while order counts scale by 10.00.
- **Recorded gap** (visible only with real data): in the two-axis/total
  render path (`src/execute/render.rs`, `value_for`/`root_value` and the
  read boundary's `unwrap_or(0.0)`), a measure whose SQL returns NULL — a
  window filter matching no rows, or a total over an empty slice — renders
  `0`, where the reference returns an empty cell and Excel shows blank. The
  1-d drilldown path instead shrinks its axis to the flagged year under
  NON EMPTY. Recorded in plan 057 for a slice of its own, with a reference
  measurement of the empty-cell encoding.
