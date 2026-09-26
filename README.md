# MallardCube

[![CI](https://github.com/felixyman/MallardCube/actions/workflows/ci.yml/badge.svg)](https://github.com/felixyman/MallardCube/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-felixyman.github.io%2FMallardCube-2f6f4f)](https://felixyman.github.io/MallardCube/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Excel pivots, against the data your pipelines already build.**

MallardCube is a single binary that speaks XMLA/SSAS to Excel. Point it at
DuckDB marts — built by sqlmesh, dbt or anything else that lands Parquet or
DuckDB files — and PivotTables, filters, drilldown, date filters and CUBE
functions work as if the data were in a cube. Your metric definitions stay
upstream, where they belong; this is a thin projection of them, not another
semantic layer. On-premises, air-gap friendly, no cloud services.

## See it work

```bash
cargo run
```

That starts the bundled demo on `http://localhost:8080/xmla` with synthetic
data. In Excel: **Data → Get Data → From Other Sources → From Analysis
Services**, server `http://localhost:8080/xmla`, catalog `SALES_ANALYTICS`,
cube `Sales` — then drag a field into a PivotTable.

The step-by-step version, including the optional `.odc` file for saving the
connection, is in [docs/EXCEL-CONNECT.md](docs/EXCEL-CONNECT.md). For a
container: `docker build -t mallardcube . && docker run -p 8080:8080 mallardcube`.

## What works

The compatibility surface is measured, not asserted: every behaviour is probed
against a real SQL Server 2025 Analysis Services instance and versioned in
[`parity/catalog.json`](parity/catalog.json), and a 1,156-request trace of a
real Excel session is replayed on every change.

- **PivotTables**: row/column fields, nested and cross-joined layouts, page
  fields, subtotals and collapse, show-values-as, number formats.
- **Filters**: label, value and Top-N — the last one is the subselect idiom
  Excel sends, evaluated the way the reference evaluates it.
- **Time**: date hierarchies, calendar levels, YTD/QTD/MTD and prior-year
  measures, Excel's date-filter dialogs.
- **CUBE functions**: `CUBEVALUE`, `CUBEMEMBER`, `CUBESET`, `CUBERANKEDMEMBER`,
  batched multi-cell formulas.
- **Drillthrough**, refresh and metadata (`MDSCHEMA_*`, `DISCOVER_*`) through the
  XMLA shapes Excel actually reads — and `DRILLTHROUGH` details on demand.
- **Security**: row-level filtering and object-level hiding from role
  configuration, with the trust boundary documented.

When the proxy cannot answer something faithfully, it says so with a fault
naming the gap — it never returns a plausible wrong number.

## How it fits

```
sqlmesh / dbt / any SQL           DuckDB or Parquet        this proxy            Excel
─ definitions and materialisation ────────────────▶  thin XMLA projection  ─────────▶  PivotTables
```

The model configuration is a projection of what upstream already knows — fact
grain, dimensions, relationships, measures, date roles — and nothing else. See
[docs/DESIGN-INVARIANTS.md](docs/DESIGN-INVARIANTS.md) for the boundary and
`projects/upstream_marts/` for a runnable thin-projection example.

## Status

Pre-alpha, and honest about it: the mirror-recorded parity catalogue is
**38/38 with no known gaps**, the per-gesture Excel certification matrix is in
progress, and unsupported shapes fault rather than guess. The [reference
site](https://felixyman.github.io/MallardCube/) publishes every behaviour claim
with its environment, method and reproduction.

## Documentation

- [The site](https://felixyman.github.io/MallardCube/) — installation,
  deployment, the Excel/SSAS behaviour reference.
- [`docs/DEVELOPER-GUIDE.md`](docs/DEVELOPER-GUIDE.md) — build, environment
  variables, engine settings, architecture.
- [`docs/OPERATIONS.md`](docs/OPERATIONS.md) — configuration walkthroughs, data
  refresh and reload, performance and scale, converting SSAS Tabular models,
  tests and gates, the security and role model.
- [`docs/EXCEL-CONNECT.md`](docs/EXCEL-CONNECT.md) — connecting Excel, and the
  optional `.odc` file.

## Contributing

Issues and pull requests are welcome. The gates a change has to keep green are
`cargo test`, `scripts/probe-fidelity.sh`, `scripts/probe-parity.sh`,
`scripts/proxy-smoke.sh` and `scripts/trace-replay.sh` — see
[`docs/OPERATIONS.md`](docs/OPERATIONS.md) for what each one covers. Licensed
under MIT.
