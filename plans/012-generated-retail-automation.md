# Plan 012: Make generated_retail_analytics low-touch under the current converter

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/convert_tabular.rs src/tools/inventory.rs generated_retail_analytics/ README.md data/retailanalytics_tabular`
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

`generated_retail_analytics` is the smallest real converted model in the repo,
so it should be the first place where “minimal manual measure work” becomes a
real product claim instead of a roadmap statement.

Right now the checked-in retail artifact is contradictory: the converter already
has lowering logic for some of the needed DAX families, but the checked-in
report still says all 4 measures are manual and the config still depends on
hand-wired fallback files. This plan uses that model as the proving ground for
the next tranche of converter automation.

## Current state (refreshed post cleanup, 2026-06-17)

- `src/tools/convert_tabular.rs` — already classifies `DIVIDE`, simple `CALCULATE`, and arithmetic-style patterns. Has lowering helpers for those families.
- `generated_retail_analytics/conversion-report.md` — now reports `Measures: 4 (simple: 1, sql_fallback: 3, manual: 0)` after manual cleanup during Plan 011 artifact normalization. Total Revenue is simple; Gross Profit, Total COGS, and Gross Margin % are SQL fallback with `ScalarOnly` capability.
- `generated_retail_analytics/proxy-config.json` — contains one simple measure (`Total Revenue`, with real `sql_expr`) and three scalar fallback measures. But these were hand-wired during Plans 010-011, not produced by the converter alone.
- `data/retailanalytics_tabular/` — the source Tabular Editor export is in-repo and reproducible.
- **The gap**: the converter does not yet reproduce this state. Its output still classifies all 4 measures as `manual`. This plan closes that gap.

Relevant excerpts:

```rust
// src/tools/convert_tabular.rs:318-343
if upper.contains("CALCULATE(") {
    if !upper.contains("ALL(") && !upper.contains("FILTER(") && !upper.contains("KEEPFILTERS") {
        return "simple".into();
    }
    return "sql_fallback".into();
}
if upper.contains("DIVIDE(") {
    return "simple".into();
}
...
"manual".into()
```

```rust
// src/tools/convert_tabular.rs:661-699
// DIVIDE(a, b) -> a / b
// CALCULATE([measure], 'dim'[col]="value") -> measure { where: ... }
```

```md
// generated_retail_analytics/conversion-report.md:5-10,22-34 (current state)
- Measures: 4 (simple: 1, sql_fallback: 3, manual: 0)
...
| Total Revenue | CALCULATE(SUM(...), Is Return = 0) | (simple sql_expr) |
| Gross Margin % | DIVIDE(...) | sql_fallback/gross_margin_pct.sql |
| Gross Profit | arithmetic | sql_fallback/gross_profit.sql |
| Total COGS | SUMX(FILTER(...), ... RELATED(...)) | sql_fallback/total_cogs.sql |
```

```json
// generated_retail_analytics/proxy-config.json:151-218 (current state)
"Gross Margin %": sql_fallback_file + ScalarOnly
"Gross Profit": sql_fallback_file + ScalarOnly
"Total COGS": sql_fallback_file + ScalarOnly
"Total Revenue": sql_expr "SUM(CASE WHEN ...)"
```
All 4 measures map correctly; none are manual. The converter must reproduce this from source.

Repo conventions to match:

- Converter improvements must be driven from real checked-in exports when available.
- Generated artifacts checked into the repo should be reproducible from the source export, not maintained by silent manual edits.
- Unsupported semantics must still fail closed; do not over-translate risky DAX.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Re-convert retail model | `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/retail-conversion` | exit 0 and writes conversion output |
| Full tests | `cargo test --lib` | all pass |
| Retail-focused tests | `cargo test --lib retail_analytics_` | all retail analytics tests pass |

## Scope

**In scope**:
- `src/tools/convert_tabular.rs`
- `src/tools/inventory.rs` if classification/reporting must stay consistent there too
- `generated_retail_analytics/`
- retail-analytics-focused tests
- `README.md` only if a user-facing status claim changes materially

**Out of scope**:
- `generated_project` healthcare fallback work
- general hierarchy expansion
- runtime execution changes outside what the converter output requires

## Steps

### Step 1: Characterize the retail model from source, not from the checked-in output alone

Run the converter against `data/retailanalytics_tabular` into a temp directory and
diff its report/config against `generated_retail_analytics/`. Identify which of the
four measures are already partially supported by the current classifier/emitter and
which still need real converter logic.

Do not start by editing the checked-in generated files by hand.

**Verify**: `cargo run --bin xmla_proxy -- convert-tabular data/retailanalytics_tabular /tmp/opencode/retail-conversion` -> exit 0.

### Step 2: Drive the exact 4 retail measures through converter-owned output

Use the checked-in retail model as the target acceptance set:

- `Total Revenue`
- `Total COGS`
- `Gross Profit`
- `Gross Margin %`

Automate only what the current runtime can support honestly:

- simple SQL/Malloy where safe
- explicit fallback metadata where required
- generated fallback SQL where safe and reproducible

The success condition is not “invent a general DAX engine.” The success
condition is that this real model becomes low-touch under the current contract.

**Verify**: regenerate to a temp directory and inspect that the emitted config/report no longer classify all 4 as manual.

### Step 3: Regenerate and check in `generated_retail_analytics/`

Once the converter output is correct, regenerate the checked-in retail artifact
from `data/retailanalytics_tabular` and update the report/config/model so the repo
contains the real converter output, not a hand-edited approximation.

**Verify**: `cargo test --lib retail_analytics_` -> all pass.

### Step 4: Add regression tests around the retail measure tranche

Add tests that prove:

- the retail conversion report/manual counts stay at the new level
- emitted fallback capability metadata matches the generated SQL contract
- the checked-in retail project still loads and executes through the existing tests

**Verify**: `cargo test --lib` -> all pass.

## Test plan

- Add converter/report characterization tests for the 4 retail measures.
- Add at least one regression asserting that the generated retail report no longer says `manual: 4`.
- Reuse the existing `retail_analytics_` project tests as the structural pattern for load/execute coverage.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] Re-converting `data/retailanalytics_tabular` produces `manual: 0` for the 4 retail measures
- [ ] `generated_retail_analytics/` is regenerated from converter output (not hand-patched)
- [ ] Retail analytics tests still pass end-to-end
- [ ] Qualify verdict matches: PARTIAL for null db_path only (no blockers, no manual measures)
- [ ] `plans/README.md` status row updated

## STOP conditions

- Correctly translating these 4 measures requires a broader DAX dependency graph or semantic compiler than the current converter architecture can support.
- The retail source export is no longer sufficient to reproduce the checked-in artifact.
- A proposed translation would silently change numeric semantics relative to the existing hand-wired retail project.

## Maintenance notes

- Reviewers should compare regenerated output against the source export, not just against the old checked-in files.
- Keep this plan anchored to the 4 real retail measures; do not expand into every possible DAX family.
- If one measure family proves unsafe, leave it manual/fallback and record that explicitly rather than guessing.
