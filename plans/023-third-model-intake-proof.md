# Plan 023: Third real model intake proof

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/convert_tabular.rs src/tools/qualify.rs src/tools/seed_generated_db.rs data/ generated_retail_analytics/ generated_project/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: `plans/022-genericize-converter-fallback-lowering.md`
- **Category**: direction
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The repo's stated production goal is proving 3 real SSAS Tabular models
migrate end-to-end. Today there are 2 checked-in converted models:

- `generated_retail_analytics` — `READY`
- `generated_project` — `PARTIAL` (only blocked by unsupported security roles)

Neither is sufficient to prove the converter is repeatable. This plan adds a
third real model through the full intake loop: convert → bootstrap → qualify →
connect Excel. The third model proves the converter is generic, not tuned for
two specific fixtures.

## Current state

- The converter lives at `src/tools/convert_tabular.rs` and accepts a
  Tabular Editor folder export (directory with `tables/`, `relationships/`,
  optional `roles/`).
- The only checked-in source export is `data/retailanalytics_tabular/`.
- `generated_project/` was converted from a source export that is no longer
  in the repo, so it cannot be regenerated from source.
- The intake loop is:
  1. `cargo run --bin xmla_proxy -- convert-tabular <source_dir> <out_dir>`
  2. `duckdb <out_dir>/data/<cube>.db < <out_dir>/bootstrap.sql`
  3. Load business data into the DuckDB tables
  4. `cargo run --bin xmla_proxy -- qualify <out_dir>/proxy-config.json`
  5. Connect Excel to the proxy

Repo conventions to match:

- Converted projects live at repo root as `generated_<name>/`.
- Source Tabular Editor exports live under `data/<name>_tabular/`.
- The `qualify` command is the readiness gate.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Convert model | `cargo run --bin xmla_proxy -- convert-tabular <source> <out>` | exit 0 |
| Bootstrap DB | `duckdb <out>/data/<cube>.db < <out>/bootstrap.sql` | exit 0 |
| Qualify | `cargo run --bin xmla_proxy -- qualify <out>/proxy-config.json` | READY or PARTIAL |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- A new `data/<name>_tabular/` source export (user-provided)
- A new `generated_<name>/` converted project
- Converter changes only if the new model exposes bugs in generic lowering
- Qualification and tests for the new model

**Out of scope**:
- Re-converting `generated_project` (source export not available)
- Security-role enforcement (Plan 024)
- Multi-level hierarchies

## Steps

### Step 1: Obtain a third real Tabular model export

The user must provide a Tabular Editor folder export for a third real SSAS
Tabular model. The export must contain:
- `tables/` with table definitions, columns, and measures
- `relationships/` with relationship definitions
- Optional: `roles/` with role definitions

Place the export under `data/<name>_tabular/`.

If no third model is available, STOP and report — this plan cannot proceed
without a real source export.

**Verify**: `ls data/<name>_tabular/tables/` -> shows table directories.

### Step 2: Convert the model

Run the converter and inspect the output:

```
cargo run --bin xmla_proxy -- convert-tabular data/<name>_tabular /tmp/opencode/model-023
```

Review the conversion report:
- How many measures are simple vs sql_fallback vs manual?
- Are the fallback SQL files real SQL or TODO stubs?
- Are the date-role and relationship detections correct?

**Verify**: conversion exits 0 and produces `proxy-config.json`,
`model.malloy`, `schema.sql`, `conversion-report.md`.

### Step 3: Bootstrap and load data

Create the DuckDB database from the generated schema and seed_date_dim:

```
duckdb /tmp/opencode/model-023/data/<cube>.db < /tmp/opencode/model-023/bootstrap.sql
```

Then load real business data into the fact and dimension tables. If real data
is not available, create a minimal seed SQL file with a few rows per table to
make the model testable.

**Verify**: `duckdb /tmp/opencode/model-023/data/<cube>.db -c "SELECT COUNT(*) FROM <fact_table>"` -> non-zero row count.

### Step 4: Qualify the model

```
cargo run --bin xmla_proxy -- qualify /tmp/opencode/model-023/proxy-config.json
```

If the verdict is `BLOCKED`, investigate which stubs are blocking and whether
the generic lowering from Plan 022 should have handled them. If the generic
lowering has a bug, fix it. If the DAX pattern is genuinely unsupported,
accept the BLOCKED status and document it.

If the verdict is `PARTIAL` or `READY`, proceed.

**Verify**: qualification produces a verdict (READY/PARTIAL/BLOCKED) without
panicking.

### Step 5: Check the model into the repo

Copy the converted project from the temp directory to `generated_<name>/`
at the repo root. Add the source export to `data/<name>_tabular/` if the user
approves.

Add at least one test that loads the new project config and verifies it
parses without errors.

**Verify**: `cargo test --lib` -> all pass, including the new model test.

### Step 6: Optional Excel smoke test

If possible, connect Excel to the proxy with the new model and verify:
- The cube appears in the connection wizard
- At least one measure returns data
- At least one dimension can be browsed

This is not a machine-checkable step, but it is the ultimate proof that the
third model works end-to-end.

## Test plan

- Add one test in `src/project/project.rs` or `src/execute/dispatch.rs` that
  loads the new project config and verifies it parses.
- Add one qualification test in `src/tools/qualify.rs` for the new model.
- If the model has fallback measures, add execution-path assertions following
  the pattern from Plan 015.

## Done criteria

- [ ] A third real Tabular model source export exists under `data/<name>_tabular/`
- [ ] `cargo run --bin xmla_proxy -- convert-tabular data/<name>_tabular generated_<name>/` exits 0
- [ ] The converted model qualifies as `READY` or `PARTIAL` (not `BLOCKED` by
      stubs that the generic lowering from Plan 022 should have handled)
- [ ] `cargo test --lib` exits 0 with at least one new test for the third model
- [ ] `plans/README.md` status row updated

## STOP conditions

- No third real Tabular model is available to convert.
- The new model exposes a converter bug that cannot be fixed without changes
  beyond the scope of this plan (e.g. a completely new DAX pattern family).
- The model's source export format is incompatible with the converter's parser.
- The model requires security-role enforcement to be usable (defer to Plan 024).

## Maintenance notes

- The third model is the strongest evidence that the converter is becoming a
  product feature, not a fixture-tuned script.
- If the third model reveals gaps in the generic lowering, those gaps should
  become new plans rather than one-off fixes.
- Keep the third model's source export checked in so the conversion is
  reproducible.
