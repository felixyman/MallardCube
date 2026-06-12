# SSAS Proxy

Excel/XMLA frontend for DuckDB-backed Malloy analytics. Runs as a local HTTP
server. Excel connects to it as a SSAS data source and gets PivotTable
compatibility — filtering, drilldown, crossjoin, collapse — against your
DuckDB data.

Malloy is the primary semantic path. Direct SQL is the automatic fallback
when Malloy compilation fails.

## Quick start

```bash
cargo run
```

Without configuration, the default `project3/` sample project (at repo root) loads with
synthetic in-memory data and starts on `http://localhost:8080/xmla`.

### With a custom project

```bash
PROXY_CONFIG=/path/to/my-project/proxy-config.json cargo run
```

### With Malloy runtime

```bash
MALLOY_RUNTIME=1 cargo run
```

When Malloy compilation fails, the request falls back to direct SQL
automatically so Excel stays functional.

## Connecting Excel

1. Start the proxy (see Quick Start above).
2. In Excel: **Data -> Get Data -> From Other Sources -> From Analysis Services**.
3. Server name: `localhost` (or `127.0.0.1`).
4. Log on credentials: **Use Windows Authentication** (no actual auth).
5. Click Finish.

The PivotTable field list shows your dimensions and measures. Filtering,
drilldown, crossjoin, and collapse all work.

### ODC file (optional)

Save an `.odc` file for repeat connections:

```xml
<html xmlns:o="urn:schemas-microsoft-com:office:office"
      xmlns="http://www.w3.org/TR/REC-html40">
<head>
<meta http-equiv="Content-Type" content="text/x-ms-odc; charset=utf-8"/>
</head>
<body>
<o:ConnectionProperties>
  <o:Name>SSAS Proxy</o:Name>
  <o:ConnectionString>Provider=MSOLAP;Data Source=http://localhost:8080/xmla;Initial Catalog=SALES_ANALYTICS</o:ConnectionString>
  <o:CommandType>Cube</o:CommandType>
  <o:CommandText>Sales</o:CommandText>
</o:ConnectionProperties>
</body>
</html>
```

## Project structure

Each project is a directory containing two files:

| File | Purpose |
|------|---------|
| `proxy-config.json` | Maps your data to Excel/XMLA: dimensions, measures, captions, formatting |
| `model.malloy` | Malloy semantic model: DuckDB table source, measure expressions |

### Config walkthrough (`proxy-config.json`)

```jsonc
{
  "catalog": "SALES_ANALYTICS",
  "cube": "Sales",
  "source_name": "sales_data",
  "table_name": "sales_fact",
  "dialect": "duckdb",
  "malloy_model_file": "model.malloy",
  "db_path": null,
  "dimensions": [
    {
      "id": "Category",
      "malloy_name": "category",
      "physical_field": "category",
      "caption": "Category",
      "hierarchy_name": "Category",
      "all_level_name": "(All)",
      "leaf_level_name": "Category",
      "ordinal": 1,
      "visible": true,
      "has_all": true,
      "cardinality_hint": 50
    }
  ],
  "measures": [
    {
      "id": "Revenue",
      "malloy_name": "total_revenue",
      "physical_expr": "revenue.sum()",
      "sql_expr": "SUM(revenue)",
      "caption": "Revenue",
      "display_name": "Revenue (USD)",
      "format_string": "#,##0.00",
      "ordinal": 1,
      "visible": true,
      "measure_group_name": "Sales"
    }
  ]
}
```

Key naming rules (see `docs/naming-contract.md`):

- **`id`** - Internal identifier for `QueryPlan`, `plan_key`, filter routing. Must be unique.
- **`malloy_name`** - Must match the field/measure name in your `.malloy` source.
- **`caption`** - Excel-visible label. Can include spaces and Unicode.

### Multi-fact-table config

For projects with more than one fact table, use the `fact_tables` array and
bind dimensions and measures to specific fact tables:

```jsonc
{
  "fact_tables": [
    { "id": "sales", "source_name": "sales_data", "table_name": "sales_fact",
      "measure_group_name": "Sales" },
    { "id": "inventory", "source_name": "inventory_data", "table_name": "inventory_fact",
      "measure_group_name": "Inventory" }
  ],
  "dimensions": [
    { "id": "Category", "shared": true, ... },
    { "id": "Channel", "fact_table": "sales", ... },
    { "id": "Warehouse", "fact_table": "inventory", ... }
  ],
  "measures": [
    { "id": "Revenue", "fact_table": "sales", ... },
    { "id": "Stock", "fact_table": "inventory", ... }
  ]
}
```

- `shared: true` dimensions apply to all fact tables.
- `fact_table` on a dimension or measure scopes it to that fact table.
- Unrelated dimension filters are silently ignored (SSAS-compatible).

### Model walkthrough (`model.malloy`)

```malloy
source: sales_data is duckdb.table('sales_fact') extend {
  measure: total_revenue is revenue.sum()
  measure: total_units is units.sum()
}
```

- The source name (`sales_data`) must match `source_name` in config.
- Dimension columns come from the DuckDB table schema.
- Declare measures with aggregation expressions.

## Demo vs Real Data

By default, the proxy runs in **demo mode**: in-memory DuckDB with synthetic
data (20k `sales_fact` rows).

To use your own DuckDB database, set `"db_path"` in `proxy-config.json` to a
file path relative to the config file:

```jsonc
{ "db_path": "../data/my-sales.db" }
```

When `db_path` is `null` or omitted, demo mode is used.

## Sample projects

Sample projects live at the repo root, sibling to `xmla_proxy/`.

| Project | Description |
|---------|-------------|
| `project/` | Minimal: 2 dims, 1 measure. Original demo. |
| `project2/` | Renamed variant proving name independence. |
| `project3/` | Wider: 4 dims, 2 measures. Default startup project. |
| `project4/` | Multi-fact: 2 fact tables (Sales + Inventory), shared and scoped dimensions. |

## Converting SSAS Tabular models

The converter turns a Tabular Editor folder (`.bim`, tables) into a Malloy +
DuckDB project:

```bash
cargo run --bin convert_tabular -- path/to/tabulareditor_src output_dir
```

Output:
- `proxy-config.json` - project config with dimensions and measures
- `model.malloy` - Malloy source with fact source and dimension joins
- `schema.sql` - DuckDB `CREATE TABLE` statements
- `sql_fallback/` - DuckDB SQL for complex measures (MEDIAN, cumulative, etc.)
- `conversion-report.md` - summary and data-loading checklist

See `docs/ssas-to-malloy-conversion.md` for details.

## Malloy runtime vs Direct SQL

| Path | Trigger | Role |
|------|---------|------|
| **Malloy** | `MALLOY_RUNTIME=1` | Long-lived Node.js worker compiles Malloy to SQL, executes via DuckDB. |
| **Direct SQL** | Always active | Rust emits SQL from `QueryPlan`. Safety net when Malloy fails. |

Both produce identical results (verified by parity tests).

## Running tests

```bash
cargo test --lib -- --test-threads=1
```

176 tests covering MDX parsing, semantic classification, plan generation, SQL
and Malloy emission, compile path, result parity, metadata rowsets, multi-fact
routing, and end-to-end cellset rendering.

Some tests spawn Node.js workers and require `--test-threads=1`.

## Architecture

For detailed documentation:

| File | Description |
|------|-------------|
| `docs/DEVELOPER-GUIDE.md` | Developer onboarding: startup flow, request lifecycle, module map |
| `docs/DIAGRAMS.md` | Mermaid diagrams (current, target, migration, collapse flow) |
| `docs/naming-contract.md` | `id` vs `malloy_name` vs `caption` naming rules |
| `docs/cellset-reference.md` | XMLA cellset layout reference |
| `docs/ssas-to-malloy-conversion.md` | Tabular `.bim` -> Malloy + DuckDB conversion reference |

## Current scope

**Works:**
- Full Excel discover/metadata handshake (all required rowsets)
- PivotTable execution: filtering, drilldown, crossjoin, collapse
- Single or multiple fact tables with shared/scoped dimensions
- One hierarchy per dimension: `(All)` + one leaf level
- Up to 2 visible row dimensions
- Malloy runtime with automatic direct-SQL fallback
- Tabular Editor `.bim` structural conversion

**Partial / in progress:**
- SQL fallback execution for converted complex measures
- Star-schema join support at runtime
- Role-playing date dimensions

**Not yet:**
- Multi-level hierarchies
- Postgres/MSSQL ingestion
- Arrow/zero-copy transport
- Security roles
