# SSAS Proxy

Excel/XMLA frontend for DuckDB. Runs as a local HTTP
server. Excel connects to it as a SSAS data source and gets PivotTable
compatibility — filtering, drilldown, crossjoin, collapse — against your
DuckDB data.

Direct SQL is the only runtime path.

## Quick start

```bash
cargo run
```

Without configuration, the default `project3/` sample project (at repo root) loads with
synthetic in-memory data and starts on `http://localhost:8080/xmla`.

The server binds to `127.0.0.1:8080` by default. To expose it on all interfaces
(e.g. for a Windows VM), set `BIND_ADDRESS=0.0.0.0:8080`. A 1 MB request body
limit is always enforced.

### With a custom project

```bash
PROXY_CONFIG=/path/to/my-project/proxy-config.json cargo run
```

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

Sample projects live at the repo root.

| Project | Description |
|---------|-------------|
| `project2/` | Renamed variant proving name independence. 2 dims, 1 measure. |
| `project3/` | Default startup. 5 dims (incl. Date with time intelligence), 6 measures (Revenue, Units, YTD, Prior Year, QTD, MTD). |
| `project4/` | Multi-fact: 2 fact tables (Sales + Inventory), shared and scoped dimensions. |
| `generated_retail_analytics/` | Converted Tabular model: 1 fact, 5 dims, 1 date-role, 4 measures (Total Revenue + 3 fallback). |

## Converting SSAS Tabular models

The converter turns a Tabular Editor folder (`.bim`, tables) into a DuckDB
project:

```bash
cargo run --bin xmla_proxy -- convert-tabular path/to/tabulareditor_src output_dir
```

Output:
- `proxy-config.json` - project config with dimensions and measures
- `schema.sql` - DuckDB `CREATE TABLE` statements
- `sql_fallback/` - DuckDB SQL for complex measures (MEDIAN, cumulative, etc.)
- `conversion-report.md` - summary and data-loading checklist

See `docs/ssas-to-malloy-conversion.md` for details.

## Running tests

```bash
cargo test --lib
```

324 tests covering MDX parsing, semantic classification, plan generation, SQL
emission, metadata rowsets, multi-fact routing, end-to-end cellset rendering,
Excel replay/oracle verification, time intelligence, security roles, and
compatibility-gate assertions.

## Compatibility gate

Every converted project should pass a structural compatibility check before
it is considered "Excel-safe." The gate verifies three layers:

1. **Discover handshake** — all required metadata rowsets return catalog, cube,
   dimension, and measure data (structurally valid XML with row elements).
2. **Execute shape** — at least one non-stub measure executes and renders a
   valid XMLA cellset (`mddataset` namespace, `<Axes>`, `<CellData>`).
3. **Replay (optional)** — when an `xmla-trace.jsonl` is available, the replay
   harness diffs captured Excel responses against live proxy output.

**Quick gate check** (against the default project):

```bash
# Record a fresh Excel session (project3 by default)
XMLA_TRACE=1 cargo run

# Replay the capture — validates discover + execute
cargo run --bin xmla_proxy -- trace-replay

# Run compatibility gate tests for generated projects
cargo test --lib retail_analytics_
```

The `trace_replay` binary validates:
- `ExecuteStatement` entries: replays MDX, diffs cell values and axis captions
- Discover/DBSCHEMA/MDSCHEMA entries: validates non-empty XML with `<row>` data
  and checks for expected catalog/cube names in key rowsets
- Session entries: validates non-empty response with standard XMLA elements

## Qualify: migration intake loop

The `qualify` subcommand gives a readiness verdict for a converted project
before you connect Excel:

```bash
cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json
cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json
```

Output: `READY`, `PARTIAL` (usable with caveats), or `BLOCKED` (stub fallbacks
or broken config — not Excel-safe). Reason codes are machine-readable.

**Full intake workflow:**

1. **Inventory** the source export:
   `cargo run --bin xmla_proxy -- inventory path/to/tabular_export/`

2. **Convert** to a MallardCube project:
   `cargo run --bin xmla_proxy -- convert-tabular path/to/tabular_export/ generated_project/`

3. **Bootstrap** the database (for projects with date-role tables):
   ```bash
   cd generated_project/
   duckdb data/<cube>.db < bootstrap.sql
   # Then load your own data into the tables listed in schema.sql
   ```

4. **Qualify** the output before Excel:
   `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json`

5. **Capture + replay** an Excel session to lock in compatibility:
   ```bash
   XMLA_TRACE=1 PROXY_CONFIG=generated_project/proxy-config.json cargo run
   # ... use Excel ...
   cargo run --bin xmla_proxy -- trace-replay xmla-trace.jsonl generated_project/proxy-config.json
   ```

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
- Direct SQL execution
- Tabular Editor `.bim` structural conversion
- Time intelligence through date-dimension flag columns: YTD, prior year, QTD, MTD
- Explicit date-role dimension (flag-based, not dynamic MDX function parsing)
- Structured fallback SQL with capability gates (scalar-only, grouped-specific, universal)
- Compatibility gate: discover + execute validation for converted projects

**Partial / in progress:**
- Fallback SQL for composite DAX measures (DIVIDE, CALCULATE, SUMX) — structural support exists, individual measure SQL must be written per model
- Star-schema join execution at runtime

**Not yet:**
- Multi-level hierarchies (Year → Quarter → Month → Day)
- Postgres/MSSQL ingestion
- Arrow/zero-copy transport

## Security and roles

The proxy supports SSAS Tabular-style role-based security via a trusted-proxy
auth boundary. Roles enforce **row-level security (RLS)** through SQL predicates,
**object-level security (OLS)** by hiding tables, and **model-level permission**
gating (read / administrator / none).

### Auth boundary

The proxy does not implement Windows Authentication (Kerberos/NTLM) natively.
Instead, it reads the authenticated user identity from a configurable HTTP
header set by a trusted reverse proxy (IIS, nginx, etc.).

```jsonc
{
  "auth": {
    "trusted_proxy": true,
    "trusted_header": "X-User"    // default
  }
}
```

- **`trusted_proxy: true`** — the proxy reads the `X-User` header (or your
  configured header name) and resolves roles against that user identity.
- **Header missing** when `trusted_proxy` is enabled → the proxy returns a 401
  (deny closed).
- **`auth` absent** (or null) → no user context is built; all requests see all
  data with administrator privileges (backward-compat mode). Roles are
  informational only.

Place IIS with Windows Authentication or nginx with a Kerberos module in front
of the proxy. The reverse proxy terminates auth and sets the trusted header
before forwarding requests.

### Role configuration

```jsonc
{
  "roles": [
    {
      "name": "EU_Sales_Managers",
      "description": "EU region sales managers — read only",
      "model_permission": "read",
      "members": [
        { "member_name": "DOMAIN\\jsmith", "member_type": "user" },
        { "member_name": "DOMAIN\\EU-Sales", "member_type": "group" }
      ],
      "table_permissions": [
        {
          "table": "sales_fact",
          "filter_expression": "f.region = 'EU'",
          "dax_filter": "Sales[Region] = \"EU\"",
          "metadata_permission": "read"
        },
        {
          "table": "dim_territory",
          "filter_expression": "_territory.region = 'EU'",
          "metadata_permission": "read"
        }
      ]
    }
  ],
  "auth": {
    "trusted_proxy": true
  }
}
```

### SQL filter contract

`filter_expression` is a **raw DuckDB SQL fragment** placed in the WHERE
clause of every query scanning that table. The table aliasing convention is:

| Table role | Alias |
|---|---|
| Fact table | `f` |
| Dimension table | `_<dimension_id>` (e.g. `_territory`, `_product`) |

Examples:

- `"f.region = 'EU'"` — filter directly on the fact table column
- `"_territory.region = 'EU'"` — filter on a joined dimension; the proxy
  cascades it through the active relationship

When the converter emits role metadata, it leaves `filter_expression` empty
and preserves the original DAX in `dax_filter`. Operators must manually
translate DAX to SQL using the aliases above.

### Semantics

| Concept | Behavior |
|---|---|
| **Multiple roles** | Union (OR) — a user in multiple roles sees the union of all rows. |
| **No matching role** (auth configured) | Deny all — empty query results, empty discover rowsets. |
| **No auth configured** | Administrator default — all data visible, roles informational. |
| **`model_permission: administrator`** | Bypasses RLS and OLS entirely. |
| **`model_permission: none`** | Deny all — empty results for all queries. |
| **No `table_permission` for a table** | Full access to that table (SSAS convention). |
| **`metadata_permission: none`** | OLS — table hidden from metadata and queries (Empty plan). |

### What is enforced

| Feature | Enforced? | Notes |
|---|---|---|
| RLS via SQL predicates | Yes | Applied as WHERE clause on every fact/dimension scan |
| Model-level read/deny | Yes | Plan returns Empty when permission is `none` |
| OLS (table-level hide) | Yes | Table hidden via Empty plan / empty member list |
| MDSCHEMA_MEMBERS filtering | Yes | Dimension table RLS applied to member enumeration |
| DAX-to-SQL lowering | No | Converter emits DAX in `dax_filter`; `filter_expression` left empty for manual fill-in |
| Column-level OLS | No | Only table-level `metadata_permission` is supported |
| Dynamic `USERNAME()` | No | `USERNAME()` and `USERPRINCIPALNAME()` are not substituted at runtime |
| Native Kerberos | No | Authentication must be terminated by a reverse proxy |

## Roadmap

The project has completed its core engineering sprint (plans 001–010). Current
direction:

1. **Prove 3 real SSAS Tabular models end-to-end** — load real customer data into
   DuckDB, convert via `convert_tabular`, pass the compatibility gate, and
   connect Excel. This surfaces model-specific blockers before generalizing.

2. **Minimal DAX measure support** — add conversion patterns for common DAX
   (DIVIDE, CALCULATE with simple filters, SUMX over single-table iterators)
   so more converted measures are executable without manual SQL.

3. **Date dimension population** — auto-generate a populated `date_dim` table
   from the converter when date-role tables are detected, so time intelligence
   works out-of-box for converted projects.

4. **Multi-level hierarchies** — only after 3 real models prove single-level
   hierarchies are sufficient for Excel browsing in practice.

Bugs and technical debt are tracked in `plans/README.md` and the
`considered and rejected` / `deferred` sections. The top 3 active concerns
are the converter measure pipeline, the bound-adapter exhaustiveness check
(previously SEC-04), and the multi-fact rendering known gap.
