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

Without configuration, the default `projects/project3/` sample project (at repo root) loads with
synthetic in-memory data and starts on `http://localhost:8080/xmla`.

The server binds to `127.0.0.1:8080` by default. To expose it on all interfaces
(e.g. for a Windows VM), set `BIND_ADDRESS=0.0.0.0:8080`. A 1 MB request body
limit is always enforced.

### With a custom project

```bash
PROXY_CONFIG=/path/to/my-project/proxy-config.json cargo run
```

### With AutoModel (zero-config)

Point the proxy at any DuckDB file — no `proxy-config.json` needed:

```bash
MALLARDCUBE_DB=/path/to/data.duckdb cargo run
```

At startup the proxy detects a fact table (largest table), measures (numeric
columns → `SUM`), dimensions (FK-linked and string columns), and date
hierarchies (`DATE` columns → a seeded `date_dim` with Year/Quarter/Month/Date),
then serves the model as cube `AutoModel`. Override the fact table with
`MALLARDCUBE_FACT=<table>`.

To generate a project you can edit instead of re-detecting every startup:

```bash
cargo run --bin xmla_proxy -- auto-model /path/to/data.duckdb --output my-project/
# writes my-project/proxy-config.json (+ bootstrap.sql for the date dimension)
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

Each project is a directory containing at minimum one file:

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
  "db_path": null,
  "dimensions": [
    {
      "id": "Category",
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

Key naming rules:

- **`id`** - Internal identifier for `QueryPlan`, `plan_key`, filter routing. Must be unique.
- **`caption`** - Excel-visible label. Can include spaces and Unicode.
- **`sql_expr`** - DuckDB SQL expression for the measure. Direct SQL is the only runtime.

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
| `projects/project2/` | Renamed variant proving name independence. 2 dims, 1 measure. |
| `projects/project3/` | Default startup. 5 dims (incl. Date with multi-level hierarchy), 6 measures (Revenue, Units, YTD, Prior Year, QTD, MTD). |
| `projects/project4/` | Multi-fact: 2 fact tables (Sales + Inventory), shared and scoped dimensions. |
| `projects/generated_retail_analytics/` | Converted Tabular model: 1 fact, 5 dims, 1 date-role, 4 real measures. Qualifies READY. |
| `projects/generated_project/` | Large healthcare model (Swedish): ~50 dims, ~80 measures. Qualifies PARTIAL (roles without auth config). |
| `projects/generated_contoso/` | Contoso retail model: 7,794 sales rows, 4 working measures, 34 helper stubs. Qualifies PARTIAL. |

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
multi-level hierarchies, DRILLTHROUGH, Excel replay/oracle verification,
time intelligence, security roles, and compatibility-gate assertions.

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
cargo run --bin xmla_proxy -- qualify projects/generated_project/proxy-config.json
cargo run --bin xmla_proxy -- qualify projects/generated_retail_analytics/proxy-config.json
```

Output: `READY`, `PARTIAL` (usable with caveats), or `BLOCKED` (stub fallbacks
or broken config — not Excel-safe). Reason codes are machine-readable.

**Full intake workflow:**

1. **Inventory** the source export:
   `cargo run --bin xmla_proxy -- inventory path/to/tabular_export/`

2. **Convert** to a MallardCube project:
   `cargo run --bin xmla_proxy -- convert-tabular path/to/tabular_export/ projects/generated_project/`

3. **Bootstrap** the database (for projects with date-role tables):
   ```bash
   cd projects/generated_project/
   duckdb data/<cube>.db < bootstrap.sql
   # Then load your own data into the tables listed in schema.sql
   ```

4. **Qualify** the output before Excel:
   `cargo run --bin xmla_proxy -- qualify projects/generated_project/proxy-config.json`

5. **Capture + replay** an Excel session to lock in compatibility:
   ```bash
   XMLA_TRACE=1 PROXY_CONFIG=projects/generated_project/proxy-config.json cargo run
   # ... use Excel ...
   cargo run --bin xmla_proxy -- trace-replay xmla-trace.jsonl projects/generated_project/proxy-config.json
   ```

## Architecture

For detailed documentation:

| File | Description |
|------|-------------|
| `docs/DEVELOPER-GUIDE.md` | Developer onboarding: startup flow, request lifecycle, module map |
| `docs/DIAGRAMS.md` | Mermaid diagrams (current, target, migration, collapse flow) |
| `docs/cellset-reference.md` | XMLA cellset layout reference |

## Current scope

**Works:**
- Full Excel discover/metadata handshake (all required rowsets)
- PivotTable execution: filtering, drilldown, crossjoin, collapse
- Multi-level date hierarchies (Year→Quarter→Month→Date expand/collapse)
- DRILLTHROUGH (double-click cell → filtered source rows)
- Single or multiple fact tables with shared/scoped dimensions
- Time intelligence through date-dimension flag columns: YTD, prior year, QTD, MTD
- Direct SQL execution (single runtime, no intermediate engine)
- Row-level security (RLS) via SQL predicates
- Object-level security (OLS) via table hiding
- Model-level permission gating (read / administrator / none)
- Trusted-proxy auth boundary (IIS/nginx → X-User header)
- Tabular `.bim` / TMDL → proxy config converter
- Structured fallback SQL with capability gates (6 generic DAX-lowering patterns)
- Qualify migration readiness gate (READY / PARTIAL / BLOCKED)
- Compatibility gate: discover + execute + replay validation
- Three converted models proven (retail, healthcare, Contoso)
- AutoModel: zero-config semantic model from any DuckDB file (`MALLARDCUBE_DB` / `auto-model` CLI)

**Partial:**
- Fallback SQL for composite DAX — 6 generic patterns covered; genuinely unsupported patterns emit honest stubs
- SSAS converter — handles common model shapes; needs manual intervention for calculation groups and complex DAX

**Not yet:**
- Attached data sources (MSSQL, Postgres, S3) — DuckDB extensions exist, not wired
- Calculation groups
- Native Kerberos — reverse proxy in front is the documented boundary

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

All 30 implementation plans complete (001–030). Next milestone is **Gate G1**
(public validation): publish the project, gather real-world usage signals, and
decide whether to proceed to Phase 4.

| # | Theme | Status |
|---|-------|--------|
| 001–003 | MDX parser → typed structural fields | DONE |
| 004–018 | Generated project intake, time intelligence, fallback SQL, converter hardening | DONE |
| 019–022 | Capability gates, CLI contract, retail stub retirement, generic DAX lowering | DONE |
| 023 | Third model intake — Contoso (4 working measures, 34 helper stubs) | DONE |
| 024–026 | Security roles: decision gate, user context, RLS/OLS enforcement | DONE |
| 027 | Drop Malloy runtime — direct SQL is the only path | DONE |
| 028 | Hygiene foundation: green baseline, lint bar, CI | DONE |
| 029 | Multi-level date hierarchies (Year→Quarter→Month→Date) | DONE |
| 030 | DRILLTHROUGH (slicer-aware "show details") | DONE |

Active concerns and deferred items are tracked in `plans/README.md`.
