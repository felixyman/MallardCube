# SSAS Proxy — Session Context

> Canonical docs: `README.md` and `docs/DEVELOPER-GUIDE.md`.
> This file is a session checkpoint, not an authoritative reference.

## Goal
- Open-source SSAS **Tabular replacement** for departmental Excel teams —
  the Excel-compatibility layer for migrating Microsoft BI stacks
  (SSIS + Kimball + SSAS) onto the modern data stack (Airflow + sqlmesh +
  DuckDB/files). *PivotTable culture survives, SSAS doesn't.*
- Backend (locked 2026-08-07): **DuckDB is the execution engine and the
  target** (in-process, zero-copy for local/file sources). External data
  via DuckDB attach extensions — parquet/CSV/S3 (`httpfs`),
  Postgres/MySQL/SQLite (official scanners), SQL Server (community `mssql`
  extension, native TDS + pushdown). `src/engine/sql.rs` stays
  DuckDB-dialect only; no second dialect ever.
- Single runtime: direct SQL only. Malloy is dropped (plan 027).
- Full direction, Gate G1 criteria, and Phase 4 candidates:
  `plans/README.md` → "Product direction (locked 2026-08-07)".

## Current state (Aug 2026)
- `src/mdx/parser.rs` — cube-agnostic nom parser; `ParsedMdx` carries axis dimensions, cube name, excluded members, collapse hierarchy as structural fields.
- `src/mdx/semantic.rs` — classification sourced from `ParsedMdx` fields (Plan 003 removed string-scan helpers).
- `src/engine/model.rs` — `SemanticModel` with multi-fact tables, relationships, `DateDimDef`, per-measure `FallbackCapability`, `UserContext`/roles, `dim_table_for_discovery()`.
- `src/engine/plan.rs` — plan generation with time-flag filter injection, fallback SQL execution with capability gates, role-gated planning (`plan_from_semantic_with_model_and_context`).
- `src/engine/sql.rs` — SQL emitter with relationship joins, date-dim subqueries, role predicates (`sql_for_query_plan_with_context`).
- `src/project/config.rs` — config schema with `time_intelligence`, `fallback_capability`, `is_date_role`, `relationships`, `roles`, `auth`.
- `src/main.rs` — axum server + clap subcommands; `build_user_context()` (trusted-header auth boundary, deny-closed).
- `src/tools/` — all tools (converter, qualify, trace_replay, data_loader, parsers, seeders); `src/bin/*.rs` are thin wrappers.
- `src/xmla_trace.rs` — NDJSON trace capture behind `XMLA_TRACE=1`.
- **Test suite: 344 passing, 1 failing** — `generated_project_fallback_measures_return_real_data` fails because `data/generated.db` (gitignored) is empty; fix is plan 028 Step 1 (`seed-generated-db`).
- **Plans 001–030 DONE.** **Gate G1 next** (public validation).
- **Three converted projects**: `generated_project` (large Swedish healthcare, PARTIAL — roles without auth config), `generated_retail_analytics` (READY), plus demo `project3`. Contoso staged at `data/contoso/` for plan 023.

## Scope boundaries (current)
- **Works**: Discover handshake, PivotTable execution (filter, drilldown,
    crossjoin, collapse), multi-level date hierarchies (Year→Quarter→
    Month→Date drilldown with expand/collapse), time intelligence via
    date-dim flags (YTD/prior/QTD/MTD), DRILLTHROUGH (slicer-aware
    "show details"), fallback SQL with capability gates, compatibility
    gate, RLS/OLS role enforcement on direct-SQL path, concurrent execution.
- **Partial**: Fallback SQL for composite DAX — 6 generic lowering patterns (MEDIAN, cumulative, SUMX+FILTER+RELATED, CALCULATE+SUM, measure arithmetic, DIVIDE); genuinely unsupported patterns emit honest stubs gated by qualify.
- **Not yet**: Attached data sources (MSSQL etc.), calculation groups, non-Excel clients.
- **Multi-level date hierarchies**: Year→Quarter→Month→Date drilldown with expand/collapse and proper SSAS-compliant cellset metadata.
- **DRILLTHROUGH**: Double-click PivotTable cells to see source rows (slicer-aware filtering).

## Key files

| Layer | File | Role |
|-------|------|------|
| HTTP + dispatch | `src/main.rs` | Axum server, `handle_xmla()`, `route_request()`, clap CLI |
| Config | `src/project/config.rs` | `ProxyConfig`, `TimeIntelligenceConfig`, `FallbackCapability`, `RoleConfig` |
| Loader | `src/project/project.rs` | `ProxyProject`, `build_semantic_model()` |
| Parser | `src/mdx/parser.rs` | Nom parser, `ParsedMdx` with structural fields |
| Semantic | `src/mdx/semantic.rs` | `SemanticQuery`, classification from `ParsedMdx` |
| Model | `src/engine/model.rs` | `SemanticModel`, `DateDimDef`, `FallbackCapability`, `UserContext` |
| Planning | `src/engine/plan.rs` | `QueryPlan`, role-gated planning, capability-gated fallback execution |
| SQL | `src/engine/sql.rs` | DuckDB-dialect emitter: joins, date-dim subqueries, role predicates |
| Runtime | `src/execute/runtime.rs` | Execution entry, timing instrumentation |
| Render | `src/execute/render.rs` | Cellset shape rendering, kind handlers |
| Axis | `src/execute/axis_members.rs` | XML axis/member helpers |
| Backend | `src/backend/mod.rs` | `QueryBackend` trait, DuckDB, demo data, date_dim seeding |
| XMLA trace | `src/xmla_trace.rs` | NDJSON capture for replay |
| Converter | `src/tools/convert_tabular.rs` | `.bim`/TMDL/folder → project config + DuckDB |
| Qualify | `src/tools/qualify.rs` | Readiness gate: READY / PARTIAL / BLOCKED |
| Replay | `src/tools/trace_replay.rs` | Compatibility gate validator |
| Fixtures | `src/test_support/fixtures.rs` | Shared MDX test constants |
| Sample | `project3/` | Default demo: 5 dims, 6 measures, time intelligence |
| Sample | `project2/`, `project4/` | Name-independence and multi-fact proofs |
| Converted | `generated_retail_analytics/` | Retail model, qualifies READY |
| Converted | `generated_project/` | Healthcare model, qualifies PARTIAL (roles) |
| Converted | `generated_contoso/` | Contoso model, qualifies PARTIAL (4 measures working, 34 helpers stub)
| Plans | `plans/` | 028, 027, 023 open; direction locked 2026-08-07 |

## Project layout
- Projects live at repo root (`project3/`, `project2/`, `project4/`, `generated_retail_analytics/`, `generated_project/`).
- Default startup: `project3/proxy-config.json` (in-memory demo).
- Set `PROXY_CONFIG` to switch. Set `XMLA_TRACE=1` for capture. Set `BIND_ADDRESS=0.0.0.0:8080` for remote access.
- Auth: optional trusted-header boundary (`auth.trusted_proxy` + `X-User`); deny-closed when enabled, admin default when absent.

## Current priorities
1. **Gate G1** — public validation before any Phase 4 epic (criteria in `plans/README.md`).

## What works today
- Full discover handshake. PivotTable execution (filter, drilldown, crossjoin, collapse). Time intelligence flag-based filtering. Multi-fact routing. Fallback SQL with capability gates. Tabular `.bim`/TMDL conversion with date-role detection and time metadata. Compatibility gate (discover + execute replay). Row- and object-level security on the direct-SQL path. Concurrent direct-SQL execution.
- 324 of 324 tests green.

## Constraints
- Excel/MSOLAP compatibility is strict. Prefer correctness over guessing.
- Fail closed on unsupported semantics (stub/unknown fallback measures return `Empty`).
- Single runtime: `QueryPlan -> SQL -> DuckDB`. No second emitter.
- Single binary, no Node.js. DuckDB is the only engine; external sources via attach extensions, never a second SQL dialect.
- Scope wall: Excel + Tabular only. Non-goals: Power BI, Multidimensional, write-back, aggregate awareness.
- Docs and plan statuses update in the same commit as the code change.
