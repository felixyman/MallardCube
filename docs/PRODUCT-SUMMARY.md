# MallardCube — Product Summary

## What it is

Open-source SSAS Tabular replacement for departmental Excel teams. Excel
connects to MallardCube as if it were an Analysis Services server, gets a
governed semantic model with dimensions/measures/security, and users work
in PivotTables as they always have. Underneath, it's DuckDB. Single binary.
No cloud, no per-user licensing, no Node.js.

**Use case:** Migrate a Microsoft BI stack (SSIS + Kimball + SSAS) onto
Airflow + sqlmesh + DuckDB — without your Excel users noticing.

## Supported

| Feature | Status |
|---|---|
| Discover handshake (all required XMLA rowsets) | ✅ |
| PivotTable execution — filter, drilldown, crossjoin, collapse | ✅ |
| Multi-level date hierarchies (Year→Quarter→Month→Date expand/collapse) | ✅ |
| Time intelligence via date-dim flags (YTD, prior year, QTD, MTD) | ✅ |
| DRILLTHROUGH — double-click cell → filtered source rows | ✅ |
| Row-level security (RLS) via SQL predicates | ✅ |
| Object-level security (OLS — table hiding) | ✅ |
| Model/role-level permissions (read / administrator / none) | ✅ |
| Trusted-proxy auth boundary (IIS/nginx → X-User header) | ✅ |
| Multi-fact tables with shared/scoped dimensions | ✅ |
| Fallback SQL for complex DAX (6 patterns: MEDIAN, cumulative, SUMX+FILTER+RELATED, CALCULATE+SUM, measure arithmetic, DIVIDE) | ✅ |
| Tabular .bim/TMDL → proxy config converter | ✅ |
| Qualify migration readiness gate (READY/PARTIAL/BLOCKED) | ✅ |
| XMLA trace capture + replay for compatibility gate | ✅ |
| Concurrent execution (direct-SQL path) | ✅ |
| Three proven converted models (healthcare, retail, Contoso) | ✅ |

## Partial

| Feature | Status |
|---|---|
| Fallback SQL for composite DAX | 6 generic patterns; genuinely unsupported patterns emit honest stubs (measures return Empty, qualify flags as BLOCKED) |
| SSAS converter | Handles common model shapes; needs manual intervention for calculation groups, column-mapped CSVs, and complex DAX |
| Non-date multi-level hierarchies | Model/code generic (any dimension can define `hierarchy_levels`), only Date is tested end-to-end |

## Not yet

| Feature | Notes |
|---|---|
| Attached data sources (MSSQL, Postgres, S3) | DuckDB attach extensions exist and are the planned path; not wired yet |
| Calculation groups | Industry-standard time intelligence mechanism (Contoso finding); accepted as a documented gap |
| Multi-level hierarchies beyond Date | Model supports it generically; needs Excel testing on non-date dimensions |
| Native Kerberos | Requires a reverse proxy (IIS/nginx) in front — documented boundary |
| Dynamic RLS (`USERNAME()` / `USERPRINCIPALNAME()`) | Not substituted at runtime. A follow-up can wire `${USER}` substitution |

## Explicitly out of scope

| Non-goal | Rationale |
|---|---|
| Power BI connectivity | Tabular metadata rowsets exist but not tested/maintained for Power BI |
| Multidimensional (MOLAP) | Tabular only |
| Write-back | Read-only runtime |
| Aggregate awareness / pre-aggregation | DuckDB's speed + departmental data sizes render this unnecessary for the target segment |
| Multi-dialect SQL emission | DuckDB only. External sources via DuckDB attach extensions |
| Cell security, column-level OLS, many-to-many relationships | Rare in departmental Tabular models |
