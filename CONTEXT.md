# SSAS Proxy — Session Context

> Canonical docs: `README.md` and `docs/DEVELOPER-GUIDE.md`.
> This file is a session checkpoint, not an authoritative reference.

## Goal
- Desktop SSAS replacement for Excel teams — Excel as primary client, DuckDB as backend, Malloy as optional semantic layer.
- Production goal: prove 3 real SSAS Tabular models migrate end-to-end with minimal manual measure work.
- Direct SQL is the safe runtime path; Malloy runtime behind `MALLOY_RUNTIME=1`.

## Current state (June 2026)
- `src/mdx/parser.rs` — cube-agnostic nom parser; `ParsedMdx` carries axis dimensions, cube name, excluded members, collapse hierarchy as structural fields.
- `src/mdx/semantic.rs` — classification sourced from `ParsedMdx` fields (Plan 003 removed string-scan helpers).
- `src/engine/model.rs` — `SemanticModel` with multi-fact tables, relationships, `DateDimDef`, per-measure `FallbackCapability`, `dim_table_for_discovery()`.
- `src/engine/plan.rs` — plan generation with time-flag filter injection, fallback SQL execution with capability gates.
- `src/engine/sql.rs` — SQL emitter with relationship joins, date-dim subqueries for time intelligence.
- `src/project/config.rs` — config schema with `time_intelligence`, `fallback_capability`, `is_date_role`, `relationships`.
- `src/bin/convert_tabular.rs` — Tabular Editor converter with fact detection, date-role detection, time metadata emission, DAX classification.
- `src/bin/trace_replay.rs` — XMLA trace replay with discover + execute validation (compatibility gate).
- `src/xmla_trace.rs` — NDJSON trace capture behind `XMLA_TRACE=1`.
- **Test suite: 221 passing tests** (zero failures, no `--test-threads=1` required).
- **Plans 001–010 complete** (see `plans/README.md`).
- **Three converted projects**: `generated_project` (large Swedish healthcare), `generated_retail_analytics` (retail star schema with real measures).

## Scope boundaries (current)
- **Works**: Discover handshake, PivotTable execution (filter, drilldown, crossjoin, collapse), 2 fact tables with shared/scoped dims, time intelligence via date-dim flags (YTD/prior/QTD/MTD), fallback SQL with capability gates, compatibility gate.
- **Partial**: Fallback SQL for composite DAX — structural support exists, individual measure SQL must be written.
- **Not yet**: Multi-level hierarchies, non-DuckDB backends, security roles.

## Key files

| Layer | File | Role |
|-------|------|------|
| HTTP + dispatch | `src/main.rs` | Axum server, `handle_xmla()`, `route_request()` |
| Config | `src/project/config.rs` | `ProxyConfig`, `TimeIntelligenceConfig`, `FallbackCapability` |
| Loader | `src/project/project.rs` | `ProxyProject`, `build_semantic_model()` |
| Parser | `src/mdx/parser.rs` | Nom parser, `ParsedMdx` with structural fields |
| Semantic | `src/mdx/semantic.rs` | `SemanticQuery`, classification from `ParsedMdx` |
| Model | `src/engine/model.rs` | `SemanticModel`, `DateDimDef`, `FallbackCapability` |
| Planning | `src/engine/plan.rs` | `QueryPlan`, `plan_from_semantic_with_model()`, `execute_plan_with_backend()` |
| SQL | `src/engine/sql.rs` | SQL emitter with joins and date-dim subqueries |
| Malloy | `src/engine/malloy.rs` | Malloy emitter |
| Runtime | `src/engine/malloy_node_longlived.rs` | Long-lived Node worker (serialized access) |
| Render | `src/execute/render.rs` | Cellset shape rendering |
| Axis | `src/execute/axis_members.rs` | XML axis/member helpers |
| Backend | `src/backend/mod.rs` | DuckDB, demo data, date_dim seeding |
| XMLA trace | `src/xmla_trace.rs` | NDJSON capture for replay |
| Converter | `src/bin/convert_tabular.rs` | Tabular `.bim` → Malloy + DuckDB |
| Replay | `src/bin/trace_replay.rs` | Compatibility gate validator |
| Fixtures | `src/test_support/fixtures.rs` | Shared MDX test constants |
| JS worker | `js/malloy-worker.js` | Long-lived Malloy compiler |
| Sample | `project3/` | Default demo: 5 dims, 6 measures, time intelligence |
| Sample | `project2/`, `project4/` | Name-independence and multi-fact proofs |
| Converted | `generated_retail_analytics/` | Real retail model with Total Revenue + fallback measures |
| Converted | `generated_project/` | Large healthcare model (many measures, relationships) |
| Plans | `plans/` | 10 implemented plans, all DONE |

## Project layout
- Projects live at repo root (`project3/`, `project2/`, `project4/`, `generated_retail_analytics/`, `generated_project/`).
- Default startup: `project3/proxy-config.json` (in-memory demo).
- Set `PROXY_CONFIG` to switch. Set `XMLA_TRACE=1` for capture. Set `BIND_ADDRESS=0.0.0.0:8080` for remote access.

## Current priorities
1. **Prove 3 real customer SSAS Tabular models end-to-end** — load data, convert, pass compatibility gate, connect Excel.
2. **Expand converter DAX classification** — DIVIDE, CALCULATE with simple filters, SUMX patterns.
3. **Auto-generate populated `date_dim` from converter** when date-role tables are detected.
4. **Multi-level hierarchies** — only after 3 real models prove single-level is sufficient for Excel browsing.

## What works today
- Full discover handshake. PivotTable execution (filter, drilldown, crossjoin, collapse). Time intelligence flag-based filtering. Multi-fact routing. Fallback SQL with capability gates. Tabular `.bim` conversion with date-role detection and time metadata. Compatibility gate (discover + execute replay).
- 221 tests, all green.

## Constraints
- Excel/MSOLAP compatibility is strict. Prefer correctness over guessing.
- Fail closed on unsupported semantics (stub/unknown fallback measures return `Empty`).
- Keep the safe runtime path (`QueryPlan -> SQL -> DuckDB`) as the default.
- Single-binary direction remains desirable.
