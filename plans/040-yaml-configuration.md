# Plan 040: Maintainable configuration — YAML, defaults, section files, fmt

## Status

- **Priority**: P2 (DX; became P1-adjacent for large models)
- **Effort**: M
- **Risk**: LOW (existing configs are unaffected; defaults only fill absent fields)
- **Depends on**: none
- **Category**: developer experience / configuration
- **Status**: **DONE 2026-09-20**

## Why this mattered

Converted models are large and the JSON config is hostile to hand maintenance:
the Contoso fixture is 722 lines for 8 dimensions and 39 measures (a
customer-scale model ran to thousands), every entry repeats the same
boilerplate (`hierarchy_name`, `all_level_name`, `leaf_level_name`, `ordinal`,
`visible`, `has_all`, `format_string`, `measure_group_name`, …), JSON has no
comments, and non-ASCII captions are escaped — so diffs and reviews are painful.

## What landed

### 1. YAML as a first-class format
`src/project/config_io.rs`: format detection by extension, then by content, for
the main config *and* section files. `ProxyProject::load` goes through the same
loader, so JSON and YAML are interchangeable (`PROXY_CONFIG=…/proxy-config.yaml`
works; `qualify` accepts it).

### 2. Derived defaults (`ProxyConfig::normalize`)
Only `id` + `caption` are required per dimension and measure. At load:

- captions cascade to `hierarchy_name`, `leaf_level_name`, `display_name`;
- `all_level_name` → `(All)`, `physical_field` → the id;
- `ordinal` follows list order; `visible` → true; `has_all` → true;
- `format_string` → `#,##0.00`;
- a measure's `measure_group_name` follows its fact table (or the cube), and a
  missing `fact_table` means the first fact table;
- `dialect` → `duckdb`, `source_name` → `table_name`.

Explicit values always win (tested).

### 3. Section files for large models
`dimensions_file`, `measures_file`, `relationships_file`, `roles_file`: each
holds the same list format, paths resolve relative to the config, inline
entries come first, and a main file may consist of little more than the
skeleton. `projects/project2/proxy-config.yaml` + `dimensions.yaml` ship as a
worked example (and a test asserts it matches the JSON fixture field by field).

### 4. Canonical writer + `mallard fmt`
`src/tools/fmt.rs` + `config_io`:

```bash
mallard fmt <config>              # rewrite canonically (format preserved)
mallard fmt --check <config>      # CI: non-zero when not canonical
mallard fmt --to yaml|json <config>  # print the effective config (stdout)
```

Canonical means: stable struct field order, defaults omitted again
(`deminimize` — a minimal config stays minimal), section files rewritten as
their own canonical lists. YAML comments are not preserved by a rewrite (serde
has no comment model); documented in README, the site, and the tool's help.

### 5. Docs
README + site model page: formats, defaults, section files, `fmt`; developer
guide: config reference table (new fields + defaults) and CLI entry.

## Scope notes

- **JSON Schema**: deferred. The structs are the schema; `fmt --check` plus the
  loader's field-named errors cover most of the value. Add a schema file if
  editor completion is wanted.
- **TOML**: out of scope (YAML covers comments and diffs).
- The repo's own fixtures are **not** formatted canonically (they predate the
  defaults and keep explicit values); `fmt` is a team tool, not a repo gate.

## Verification

- `config_io` tests (9): format detection, YAML/JSON equivalence, derived
  defaults, explicit values never overwritten, section merge order, canonical
  round-trip + dirty detection, split write/reload, JSON stays JSON, and the
  shipped project2 YAML twin matches its JSON fixture.
- Live: `qualify projects/project2/proxy-config.yaml` → PARTIAL (demo `db_path`,
  as expected); `mallard fmt --to yaml projects/project3/proxy-config.json`
  prints compact YAML; a formatted JSON copy still qualifies.
- 437 tests green, clippy/fmt clean, smoke 8/8, site builds.
