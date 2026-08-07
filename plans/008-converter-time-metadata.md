# Plan 008: Teach the Tabular converter to emit runtime time metadata

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat a1b1bd4..HEAD -- src/bin/convert_tabular.rs src/bin/inventory.rs docs/ssas-to-malloy-conversion.md generated_project/proxy-config.json generated_project/conversion-report.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/006-explicit-time-contract.md`, `plans/007-measure-scoped-date-roles.md`
- **Category**: direction
- **Planned at**: commit `a1b1bd4`, 2026-06-16

## Why this matters

The runtime now understands time-intelligence metadata, but the converter still
emits old-style output: date-role tables become plain dimensions and
`TOTALYTD` / `SAMEPERIODLASTYEAR` measures become `sql_fallback` stubs. That
means the most realistic path into the product still drops users back into
manual cleanup.

The next step is to make converted projects land on the runtime contract the
proxy already understands, so a `.bim` import can become a usable Excel model
instead of a partially translated scaffold.

## Current state

- `src/bin/convert_tabular.rs` — classifies DAX and renders `proxy-config.json`.
- `generated_project/proxy-config.json` — current output fixture from the converter.
- `docs/ssas-to-malloy-conversion.md` — documented target behavior.
- `generated_project/conversion-report.md` — current evidence of 7 date roles and 11 fallback measures.

Relevant excerpts:

```rust
// src/bin/convert_tabular.rs:291-297
if upper.contains("ALLSELECTED") || upper.contains("ISONORAFTER") || upper.contains("TOTALYTD") || upper.contains("DATESYTD") {
    return "sql_fallback".into();
}
...
if upper.contains("TODAY()") || upper.contains("NOW()") || upper.contains("UTCNOW()") || upper.contains("SAMEPERIODLASTYEAR") {
    return "sql_fallback".into();
}
```

```rust
// src/bin/convert_tabular.rs:344-365
format!(
  r##"{{
  \"catalog\": "{catalog}",
  ...
  \"relationships\": [ ... ],
  \"dimensions\": [ ... ],
  \"measures\": [ ... ]
}}"##
)
```

```json
// generated_project/proxy-config.json:1-8
{
  "catalog": "SEMANTICMODEL",
  "cube": "DW_FYS_F_UNDERSÖKNING",
  ...
  "db_path": "data/generated.db",
```

There is no top-level `time_intelligence` block and no emitted date-role/time
metadata despite the date-role relationships that follow.

```md
// docs/ssas-to-malloy-conversion.md:417-419
DAX time-intelligence functions (`TOTALYTD`, `SAMEPERIODLASTYEAR`, `DATESYTD`, etc.)
do not map to relational queries. Instead, model a date dimension table with
pre-built columns, then filter on those columns in Malloy measures.
```

Repo conventions to match:

- The converter is allowed to emit partial support, but it should say so in
  `conversion-report.md` rather than silently producing a misleading runtime contract.
- Keep generated JSON human-editable and consistent with the checked-in sample projects.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build | `cargo build --bin convert_tabular` | exit 0 |
| Converter tests | `cargo test --bin convert_tabular` | all pass |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/bin/convert_tabular.rs`
- `src/bin/inventory.rs` (only if the emitted/report terminology must stay aligned)
- `docs/ssas-to-malloy-conversion.md`
- generated output fixtures checked into `generated_project/`
- converter-focused tests

**Out of scope**:
- runtime planner/executor changes beyond what Plans 006-007 already define
- generic MDX time-function parsing
- fully solving converted complex measures; Plan 010 covers that

## Steps

### Step 1: Emit date-role metadata for converted dimensions

Update `render_dimension_configs()` so converted date-role tables are emitted
with the runtime metadata they now deserve:

- `is_date_role: true`
- the correct date-role binding shape for Plans 006-007
- stable explicit physical fields/table references

Keep regular dimensions unchanged.

**Verify**: add/update converter tests and run `cargo test --bin convert_tabular` -> pass.

### Step 2: Emit time-intelligence metadata for supported DAX patterns

For the simplest supported patterns (`TOTALYTD`, `DATESYTD`,
`SAMEPERIODLASTYEAR`, and equivalent current-year cumulative flags if present):

- stop classifying them as generic `sql_fallback` when they can be expressed
  through the runtime time model
- emit the correct per-measure time metadata instead
- preserve unsupported dynamic patterns (`DATESBETWEEN`, arbitrary FILTER
  windows, etc.) as fallback/manual output

Do not over-claim support. Unsupported patterns must stay clearly labeled.

**Verify**: regenerate a small fixture and confirm the emitted JSON contains
time metadata for supported patterns and fallback files only for unsupported ones.

### Step 3: Emit top-level time-intelligence scaffolding

Make `render_proxy_config()` emit the top-level time-intelligence block needed
by the runtime, including the date-role/default calendar metadata required by
Plans 006-007.

If converter output cannot infer a safe default automatically, emit a warning in
`conversion-report.md` instead of guessing.

**Verify**: `cargo build --bin convert_tabular` -> exit 0.

### Step 4: Update the checked-in generated project fixture

Regenerate or manually refresh `generated_project/proxy-config.json` and
`generated_project/conversion-report.md` so the checked-in sample reflects the
new converter output.

The report should explicitly separate:

- runtime-supported flag-based time measures
- fallback measures still needing raw SQL

**Verify**: `cargo test --lib generated_project` -> generated-project tests pass.

### Step 5: Align the conversion guide with the emitted contract

Update `docs/ssas-to-malloy-conversion.md` so its examples and the actual
converter output use the same field names and same support boundaries.

**Verify**: `grep -n "time intelligence" docs/ssas-to-malloy-conversion.md` shows the updated field names and boundaries.

## Test plan

- Add converter tests for supported time-intelligence DAX patterns.
- Add fixture assertions on emitted JSON.
- Add at least one regression test proving a supported YTD measure no longer
  lands on `sql_fallback_file` output.
- Re-run generated-project loader tests after fixture refresh.

## Done criteria

- [ ] `cargo build --bin convert_tabular` exits 0
- [ ] `cargo test --bin convert_tabular` exits 0
- [ ] At least one supported time-intelligence measure is emitted as runtime metadata, not `sql_fallback`
- [ ] `generated_project/proxy-config.json` contains emitted date-role/time metadata
- [ ] `conversion-report.md` distinguishes supported runtime time measures from remaining fallbacks
- [ ] `plans/README.md` status row updated

## STOP conditions

- A supported DAX pattern cannot be mapped to the runtime time contract without
  guessing the date role.
- Regenerating `generated_project` would overwrite user-owned manual edits that
  are not reproducible from the converter.
- The runtime contract from Plans 006-007 is not yet stable enough to emit.

## Maintenance notes

- Keep the converter conservative. It is better to emit a clearly unsupported
  fallback than to emit wrong runtime metadata.
- Reviewers should inspect regenerated fixtures carefully; they are a user-facing contract.
