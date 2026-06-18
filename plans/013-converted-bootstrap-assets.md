# Plan 013: Generate runnable bootstrap assets for converted projects

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/convert_tabular.rs src/project/config.rs src/project/project.rs data/ README.md generated_project/ generated_retail_analytics/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: `plans/011-converted-project-qualification.md`
- **Category**: direction
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The repo can now convert models, emit time metadata, and replay Excel traffic.
What still slows real-model proof is the manual “now load everything into
DuckDB yourself” step after conversion.

That friction is visible in both checked-in converted projects and directly cuts
against the goal of proving 3 real models end-to-end. This plan makes converted
projects come with runnable bootstrap assets, especially for date-role/time-
intelligence scenarios.

## Current state

- `src/tools/convert_tabular.rs` — always emits `"db_path": null` and currently stops at `schema.sql`, `model.malloy`, `proxy-config.json`, `conversion-report.md`, and optional `sql_fallback/` files.
- `src/tools/convert_tabular.rs` — emits a `time_intelligence.date_dimension` block when a date-role table exists, but does not emit populated calendar data assets.
- `generated_retail_analytics/conversion-report.md` — tells operators to load all tables manually.
- `generated_project/conversion-report.md` — tells operators to load 17 M-partition tables manually even though the runtime now understands date-role metadata.
- `generated_project/proxy-config.json` — uses a hand-set `db_path` (`data/generated.db`) to support the current smoke fixture, proving that bootstrap assets matter in practice.

Relevant excerpts:

```rust
// src/tools/convert_tabular.rs:375-406
"malloy_model_file": "model.malloy",
"db_path": null,
...
"relationships": [...],
{ti}
"dimensions": [...],
"measures": [...]
```

```rust
// src/tools/convert_tabular.rs:455-465
// Use the first date-role dimension as the default calendar dimension.
"time_intelligence": {
  "date_dimension": {
    "dimension_id": "...",
    "table_name": "...",
    "date_key_column": "date_key",
    ...
  }
}
```

```md
// generated_retail_analytics/conversion-report.md:41-57
All tables use M (Power Query) partitions and must be loaded into DuckDB manually.
```

```md
// generated_project/conversion-report.md:76-103
All tables use M (Power Query) partitions and must be loaded into DuckDB manually.
...
- [ ] `dw_fys_kalender_undersökningsslut` (date-role)
```

Repo conventions to match:

- Keep the safe runtime path (`QueryPlan -> SQL -> DuckDB`) as the default.
- Generated artifacts should be explicit; hidden smoke-only mutations are harder to trust than named bootstrap files.
- Time intelligence in this repo is data-modeled through date-dimension flags, not through runtime MDX time semantics.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Convert retail model | `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/bootstrap-retail` | exit 0 |
| Seed generated smoke DB | `cargo run --bin xmla_proxy -- seed-generated-db` | exit 0 |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/tools/convert_tabular.rs`
- `src/project/config.rs` and `src/project/project.rs` only if bootstrap metadata needs schema support
- generated bootstrap assets under `data/` or inside converted project directories
- `generated_project/`
- `generated_retail_analytics/`
- `README.md`

**Out of scope**:
- full generic ETL/import tooling for arbitrary external data sources
- hierarchy work
- DAX translation beyond what bootstrap/date assets require

## Steps

### Step 1: Define the bootstrap asset contract

Decide what a converted project should ship so an operator can get to a runnable
DuckDB state without inventing the next step. At minimum, cover:

- where the DuckDB file should live (or how it is named)
- how date/calendar data is materialized for time-intelligence support
- how the repo distinguishes pure conversion output from runnable smoke assets

Keep the contract explicit and file-based.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Extend the converter to emit those bootstrap assets

Teach the converter to emit the chosen bootstrap files for projects with date-role
metadata and M-partition/manual-loading requirements. Focus on deterministic,
operator-readable assets such as SQL/bootstrap scripts and clearly named paths.

Do not try to solve generic data extraction from SSAS sources here; stay at the
level of reproducible DuckDB-side bootstrap assets.

**Verify**: `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/bootstrap-retail` -> exit 0 and the new bootstrap files exist.

### Step 3: Apply the contract to the checked-in converted projects

Update the checked-in converted artifacts so they demonstrate the new contract.
That may mean regenerating project output, moving smoke-only overrides into
named files, or updating docs/assets around `generated_project` and
`generated_retail_analytics`.

The key requirement is clarity: a maintainer should be able to tell which files
are converter output and which are runnable bootstrap helpers.

**Verify**: converted project directories contain the declared bootstrap assets and no longer rely on undocumented manual steps alone.

### Step 4: Document the bootstrap path in README

Add a short operator workflow for converted projects that answers:

- what the converter emits
- how to create/open the DuckDB database
- how date/time-intelligence data is supplied
- how to start the proxy against the converted project

**Verify**: `grep -n "bootstrap\|date_dim\|db_path" README.md` -> shows the new workflow.

## Test plan

- Add converter regression coverage that asserts bootstrap assets are emitted.
- Add at least one project-loading test proving the new converted bootstrap path is recognized.
- Keep the existing `generated_project` smoke path green.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] Converting a real checked-in model emits explicit bootstrap assets
- [ ] The converted bootstrap path includes populated date/time-intelligence support where date roles exist
- [ ] README documents the converted-project bootstrap flow
- [ ] `plans/README.md` status row updated

## STOP conditions

- The source export does not provide enough metadata to synthesize a safe date/bootstrap asset contract.
- The only viable implementation requires a full external-data ingestion system rather than file-based bootstrap assets.
- The plan would force misleading `db_path` assumptions into pure converter output without making the smoke/runtime distinction explicit.

## Maintenance notes

- Reviewers should be wary of “magic” bootstrap behavior; generated files should be obvious and inspectable.
- Keep the bootstrap contract narrow and DuckDB-focused.
- This plan is about making converted projects runnable, not about replacing upstream ETL.
