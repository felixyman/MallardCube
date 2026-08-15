# Converting SSAS Tabular models

The `convert-tabular` command turns a Tabular Editor export (`.bim`, TMDL, or a
folder of tables) into a MallardCube project.

> Commands below assume the `mallard` CLI is on your `PATH` (`cargo install
> mallardcube`). From a checkout, prefix with `cargo run --bin mallard --`.

```bash
mallard convert-tabular path/to/tabulareditor_src output_dir
```

Output:

- `proxy-config.json` — project config with dimensions and measures
- `schema.sql` — DuckDB `CREATE TABLE` statements
- `sql_fallback/` — DuckDB SQL for complex measures (MEDIAN, cumulative, etc.)
- `conversion-report.md` — summary and data-loading checklist

## Migration intake loop

The full flow for bringing an existing Tabular model into MallardCube:

1. **Inventory** the source export:

   ```bash
   mallard inventory path/to/tabular_export/
   ```

2. **Convert** to a MallardCube project:

   ```bash
   mallard convert-tabular path/to/tabular_export/ projects/generated_project/
   ```

3. **Bootstrap** the database (for projects with date-role tables):

   ```bash
   cd projects/generated_project/
   duckdb data/<cube>.db < bootstrap.sql
   # Then load your own data into the tables listed in schema.sql
   ```

4. **Qualify** the output before Excel:

   ```bash
   mallard qualify projects/generated_project/proxy-config.json
   ```

   Output: `READY`, `PARTIAL` (usable with caveats), or `BLOCKED` (stub
   fallbacks or broken config — not Excel-safe). Reason codes are
   machine-readable.

5. **Capture + replay** an Excel session to lock in compatibility:

   ```bash
   XMLA_TRACE=1 PROXY_CONFIG=projects/generated_project/proxy-config.json mallard
   # ... use Excel ...
   mallard trace-replay xmla-trace.jsonl projects/generated_project/proxy-config.json
   ```

## Compatibility gate

Every converted project should pass a structural compatibility check before it
is considered "Excel-safe." The gate verifies three layers:

1. **Discover handshake** — all required metadata rowsets return catalog, cube,
   dimension, and measure data (structurally valid XML with row elements).
2. **Execute shape** — at least one non-stub measure executes and renders a
   valid XMLA cellset (`mddataset` namespace, `<Axes>`, `<CellData>`).
3. **Replay (optional)** — when an `xmla-trace.jsonl` is available, the replay
   harness diffs captured Excel responses against live proxy output.

**Quick gate check** (against the default project):

```bash
# Record a fresh Excel session (project3 by default)
XMLA_TRACE=1 mallard

# Replay the capture — validates discover + execute
mallard trace-replay

# Run compatibility gate tests for generated projects
cargo test --lib retail_analytics_
```

The `trace-replay` command validates:

- `ExecuteStatement` entries: replays MDX, diffs cell values and axis captions
- Discover/DBSCHEMA/MDSCHEMA entries: validates non-empty XML with `<row>` data
  and checks for expected catalog/cube names in key rowsets
- Session entries: validates non-empty response with standard XMLA elements
