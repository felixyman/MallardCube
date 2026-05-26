# SSAS Proxy

Excel/XMLA frontend for DuckDB-backed Malloy analytics. Runs as a local HTTP
server. Excel connects to it as a SSAS data source and gets PivotTable
compatibility — filtering, drilldown, crossjoin, collapse — against your own
DuckDB data.

Malloy is the primary semantic path. Direct SQL runs as the automatic
fallback when Malloy compilation fails.

## Quick start

```bash
cargo run
```

That's it. Without configuration, the default `project3/` sample project loads:

```
Project loaded: SALES_ANALYTICS | cube=Sales | 4 dims, 2 measures
```

The server starts on `http://localhost:8001/msmdpump.dll`.

### With a custom project

```bash
PROXY_CONFIG=/path/to/my-project/proxy-config.json cargo run
```

### With Malloy runtime

```bash
MALLOY_RUNTIME=1 cargo run
```

When Malloy compilation fails (missing column, syntax error), the request
automatically falls back to direct SQL so Excel stays functional.

## Project structure

Each project is a directory containing two files:

| File | Purpose |
|------|---------|
| `proxy-config.json` | Maps your data to Excel/XMLA: dimensions, measures, captions, formatting |
| `model.malloy` | Malloy semantic model: DuckDB table source, dimension columns, measure expressions |

### Config walkthrough (`proxy-config.json`)

```jsonc
{
  "catalog": "SALES_ANALYTICS",    // Excel-visible catalog name
  "cube": "Sales",                 // Cube name
  "source_name": "sales_data",     // Malloy source name (must match .malloy file)
  "table_name": "sales_fact",      // DuckDB table name
  "dialect": "duckdb",             // Backend dialect
  "malloy_model_file": "model.malloy",
  "db_path": null,                 // Optional: path to DuckDB file. null = demo mode.
  "dimensions": [
    {
      "id": "Category",            // Internal identifier for QueryPlan / plan_key
      "malloy_name": "category",   // Name in Malloy model and DuckDB column
      "physical_field": "category",// DuckDB column name
      "caption": "Category",       // Excel-visible label
      "hierarchy_name": "Category",// SSAS hierarchy name
      "all_level_name": "(All)",
      "leaf_level_name": "Category",
      "ordinal": 1,
      "visible": true,
      "has_all": true
    }
  ],
  "measures": [
    {
      "id": "Revenue",
      "malloy_name": "total_revenue",     // Measure name in Malloy model
      "physical_expr": "revenue.sum()",   // Malloy expression
      "sql_expr": "SUM(revenue)",         // SQL fallback expression
      "caption": "Revenue",
      "display_name": "Revenue (USD)",    // Longer Excel label
      "format_string": "#,##0.00",
      "ordinal": 1,
      "visible": true,
      "measure_group_name": "Sales"
    }
  ]
}
```

Key naming rules (see `docs/naming-contract.md` for details):

- **`id`** — Internal identifier used in `QueryPlan`, `plan_key`, filter routing.
  Must be unique within the project.
- **`malloy_name`** — Must match the corresponding field/measure name in your
  `.malloy` source. Stored as `semantic_name` in the model.
- **`caption`** — Excel-visible label. Can be anything, including spaces and
  Unicode. No requirement to match `id` or `malloy_name`.

### Model walkthrough (`model.malloy`)

```malloy
source: sales_data is duckdb.table('sales_fact') extend {
  measure: total_revenue is revenue.sum()
  measure: total_units is units.sum()
}
```

- The source name (`sales_data`) must match `source_name` in config.
- The table name (`sales_fact`) must match `table_name` in config.
- Dimension columns are auto-detected from the DuckDB table — you only need
  to declare dimension definitions in the config (or in Malloy when renaming).
- Measure definitions declare aggregation expressions.

## Connecting Excel

1. Start the proxy (see Quick Start above).
2. In Excel: **Data → Get Data → From Other Sources → From Analysis Services**.
3. Server name: `localhost` (or `127.0.0.1`).
4. Log on credentials: **Use Windows Authentication** (no actual auth).
5. Click Finish — Excel discovers the cube and dimensions.

The PivotTable field list should show your dimensions and measures.
Filtering, drilldown, crossjoin, and collapse should all work.

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
  <o:ConnectionString>Provider=MSOLAP;Data Source=http://localhost:8001/msmdpump.dll;Initial Catalog=SALES_ANALYTICS</o:ConnectionString>
  <o:CommandType>Cube</o:CommandType>
  <o:CommandText>Sales</o:CommandText>
</o:ConnectionProperties>
</body>
</html>
```

## Malloy runtime vs Direct SQL

The proxy has two analytic paths:

| Path | Trigger | Role |
|------|---------|------|
| **Malloy** | `MALLOY_RUNTIME=1` env var | Primary semantic path. Long-lived Node.js worker compiles Malloy source to SQL, then executes via DuckDB. |
| **Direct SQL** | Always available | Reference/fallback. Rust emits SQL directly from the `QueryPlan`. Automatically engaged when Malloy compile fails. |

Both paths produce identical results (verified by parity tests). Malloy is the
strategic direction; direct SQL serves as the debugging oracle and safety net.

Caches exist for Malloy source text, SQL text, and compiled SQL — normalized
via `plan_key(plan)`.

## Demo vs Real Data

By default, the proxy runs in **demo mode**: it creates an in-memory DuckDB
database with synthetic data (20k `sales_fact` rows). This lets you explore
the proxy without setting up any data.

To use your own DuckDB database:

1. Create or point to a DuckDB file with your fact table.
2. Set `"db_path"` in `proxy-config.json` to the file path (relative to the
   config file):

```jsonc
{
  "db_path": "../data/my-sales.db",
  // ...
}
```

3. Start the proxy normally. If `MALLOY_RUNTIME=1`, both the Rust backend and
   the Malloy JS worker open the **same** DuckDB file. Malloy compiles against
   the real schema — no fake schema derivation needed.

When `db_path` is `null` (or omitted), the proxy uses demo mode
(in-memory database with synthetic data).

## Running tests

```bash
cargo test --lib -- --test-threads=1
```

152 tests covering MDX parsing, semantic classification, plan generation, SQL
emission, Malloy emission, compile path, result parity, metadata rowsets, and
end-to-end cellset rendering.

Some tests spawn Node.js workers and require `--test-threads=1` for
serialization.

## Architecture

For architecture diagrams, naming convention details, and cellset reference:

| File | Description |
|------|-------------|
| `docs/DIAGRAMS.md` | Mermaid diagrams (current, target, migration, collapse flow) |
| `docs/naming-contract.md` | `id` vs `malloy_name` vs `caption` naming rules |
| `docs/cellset-reference.md` | XMLA cellset layout reference |
| `docs/ssas-to-malloy-conversion.md` | SSAS Tabular `.bim` → Malloy + DuckDB conversion reference |

## Supported scope

- One DuckDB fact source with flat dimension columns
- One hierarchy per dimension: `(All)` + one leaf level
- Aggregate measures via Malloy expressions
- Up to 2 visible row dimensions for Excel PivotTable interactions

Not yet: multi-level hierarchies, multi-source joins, Postgres/MSSQL
ingestion, Arrow/zero-copy transport.
