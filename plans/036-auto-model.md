# Plan 036: AutoModel — zero-config semantic model from any DuckDB

## Status

- **Priority**: P3 (adoption, large effort)
- **Effort**: L
- **Risk**: MEDIUM
- **Depends on**: Gate G1 success (public validation signals demand)
- **Category**: adoption

## Why this matters

The current onboarding flow requires a `proxy-config.json` — either hand-written
or converter-generated from a `.bim` file. This assumes the user has SSAS
artifacts to convert. An analyst with a DuckDB file and no SSAS background has
no path in.

AutoModel drops the prerequisite. Point MallardCube at any DuckDB file (or
in-memory dataset) and it auto-detects dimensions, measures, relationships,
date hierarchies, and exposes a PivotTable-ready semantic model with zero
configuration.

This is the onboarding wedge: from zero to working PivotTable in the time it
takes DuckDB to open the file.

## Design

### Detection heuristics

| Concept | Heuristic | Fallback |
|---|---|---|
| **Fact table** | Table with most rows AND most numeric columns. If multiple, pick largest. | Override: `MALLARDCUBE_FACT=<table>` |
| **Measures** | All numeric columns in the fact table. Default aggregation: `SUM`. | Override: config `measures` |
| **Dimensions** | String/varchar columns on joined tables where `PRAGMA foreign_keys` shows an FK from the fact table. Also columns on the fact table itself (degenerate dims). | If no FKs: all non-numeric columns in all tables |
| **Date hierarchies** | Any column with DATE/TIMESTAMP type. Auto-create Year/Quarter/Month/Day levels via `date_dim` generation. | Single-level if date_dim can't be generated |
| **Relationships** | `PRAGMA foreign_keys` for declared FKs. For undeclared: column name heuristics (`*_id`, `*_key` matching `id` in another table). | One-to-many by cardinality sampling |

### Generated config

```jsonc
{
  "catalog": "AUTO_DETECTED",
  "cube": "AutoModel",
  "table_name": "sales_fact",        // auto-detected
  "db_path": "data/sales.duckdb",
  "dimensions": [ ... ],             // auto-detected
  "measures": [ ... ],               // auto-detected
  "dialect": "duckdb",
  "auto_model": true                  // flag: re-detect on restart
}
```

### Date dimension generation

When a DATE/TIMESTAMP column is detected on the fact table:
1. Generate `date_dim` with `generate_series` from `MIN(date)` to `MAX(date)`
2. Populate Year/Quarter/Month/Day columns
3. Add hierarchy_levels for the Date dimension
4. Create a relationship `fact.date_key → date_dim.full_date`

### Bootstrap flow

```bash
# Option A: existing DuckDB
MALLARDCUBE_DB=data/sales.duckdb cargo run
# → generates auto-model, connects Excel, works immediately

# Option B: CSV/Parquet via httpfs
MALLARDCUBE_DB='s3://bucket/sales.parquet' cargo run
# → DuckDB httpfs attaches the file, auto-model runs

# Option C: save the generated config for customization
cargo run --bin xmla_proxy -- auto-model data/sales.duckdb --output my-project/
# → writes proxy-config.json, then user can edit
```

### CLI

```bash
cargo run --bin xmla_proxy -- auto-model <db_path> [--output <dir>]
```

Outputs a `proxy-config.json` and optionally a `bootstrap.sql` for the date_dim.

### Safe defaults

- All dimensions visible, all measures visible
- No security roles
- No fallback SQL — all measures use `SUM(col)` or `COUNT(col)`
- Catalog name from db filename: `sales.duckdb` → catalog `sales`

## Scope

**In scope:**
- Fact table detection (row count + numeric column density)
- Measure detection (all numeric fact columns → SUM/COUNT)
- Dimension detection (FK resolution via `PRAGMA foreign_keys` + column name
  heuristics)
- Date dimension generation with hierarchy_levels
- `auto-model` CLI subcommand
- Runtime auto-detection mode (`auto_model: true` in config or env var)

**Out of scope:**
- DAX/measure expression detection (SQL only — SUM/COUNT/AVG defaults)
- Calculation groups
- Entity-relationship diagram from M/L query files
- Interactive schema editor
- Incremental model refresh

## Risks

| Risk | Mitigation |
|---|---|
| Wrong fact table detected | Override via env/config |
| Foreign keys not declared in DuckDB | Column name heuristics (`*_id` pattern) cover 80% of cases |
| Bad measure defaults (SUM on a ratio column) | All measures visible by default; user hides in Excel field list |
| Date hierarchy performance on large date ranges | `generate_series` is fast; 50 years = 18K rows |

## Deferred from

This plan is deferred until **after Gate G1**. AutoModel only makes sense if the
project attracts non-SSAS-conversion users — i.e., analysts who have DuckDB data
and want Excel PivotTables without SSAS. Gate G1 signals whether that segment
exists.

## Done criteria

- [ ] `cargo run --bin xmla_proxy -- auto-model data/sales.duckdb` produces a
      valid `proxy-config.json`
- [ ] Auto-detected model connects Excel and shows measures + dimensions
- [ ] Date columns auto-generate multi-level hierarchies
- [ ] `MALLARDCUBE_DB` env var triggers auto-model at startup
- [ ] Manual override of fact table, measures, dims via config
