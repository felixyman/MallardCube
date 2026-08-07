# Plan 028: Hygiene foundation — green baseline, plan bookkeeping, lint bar, CI

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f3837cd..HEAD -- src/execute/dispatch.rs src/tools/seed_generated_db.rs src/tools/qualify.rs src/main.rs src/engine/plan.rs src/engine/sql.rs plans/README.md README.md CONTEXT.md docs/DEVELOPER-GUIDE.md data/seed_generated.sql`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none (execute first, before 027 and 023)
- **Category**: hygiene
- **Planned at**: commit `f3837cd`, 2026-08-07

## Why this matters

Three kinds of drift accumulated while feature work continued:

1. **A red test from a missing local fixture** — the only execution-proof
   test for the most customer-shaped model depends on a gitignored DuckDB
   file that silently degrades to an empty database.
2. **Plan bookkeeping disagrees with code** — plans 024 and 026 are
   implemented but still marked TODO; three docs carry stale test counts;
   the developer guide describes the pre-reorg module layout.
3. **No automated gate** — no CI; 157 clippy warnings; ~14 dead functions
   left over from the file reorg.

The Malloy excision (027) and Contoso intake (023) both need a trustworthy
baseline and an automatic gate. This plan establishes both.

## Current state (verified 2026-08-07)

- `cargo test --lib` -> **344 passed, 1 failed**:
  `execute::dispatch::tests::generated_project_fallback_measures_return_real_data`
  fails with `Catalog Error: Table with name dw_fys_f_undersökning does not
  exist!` — `data/generated.db` (gitignored) is an empty 12 KB file.
- Seed tool exists: `cargo run --bin xmla_proxy -- seed-generated-db`
  (`src/tools/seed_generated_db.rs`) deletes and recreates
  `data/generated.db` from `data/seed_generated.sql` and prints fact/dim
  row counts.
- `plans/README.md` rows 024 and 026 say TODO, but the code implements
  both: `build_user_context` (`src/main.rs:251`),
  `plan_from_semantic_with_model_and_context` (`src/engine/plan.rs:156`),
  `sql_for_query_plan_with_context` (`src/engine/sql.rs:28`), qualify role
  gate message (`src/tools/qualify.rs:103`), role-filter E2E regression
  test (`src/execute/runtime.rs:223`). README.md documents the full
  security feature ("Security and roles" section).
- Stale docs:
  - `README.md` test suite section says "234 tests".
  - `CONTEXT.md` says "234 passing tests" (line ~21) and "221 tests, all
    green" (What works today) and "Plans 001–019 complete".
  - `docs/DEVELOPER-GUIDE.md` module map predates the reorg: describes
    `execute/builders.rs` as "too large, mixed responsibilities" (it is now
    a 122-line shim), omits `src/execute/runtime.rs` and `src/tools/`,
    and its tool list misses `qualify` and `load_replay`.
- `cargo clippy --lib` -> **157 warnings** (102 auto-fixable: collapsible
  ifs, doc-comment blank lines, needless borrows); `cargo build` -> 29
  warnings; ~14 never-used functions (e.g. `full_slicer_axis`,
  `all_member_for` in `src/execute/axis_members.rs`, `extract_tag`,
  `is_valid_uuid` in `src/xmla/discover/members.rs`).
- No `.github/workflows/` — tests run only when someone remembers.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Re-seed fixture | `cargo run --bin xmla_proxy -- seed-generated-db` | prints non-zero fact/dim counts, exit 0 |
| Full tests | `cargo test --lib` | 0 failures |
| Clippy autofix | `cargo clippy --fix --allow-dirty --allow-staged` | compiles |
| Lint check | `cargo clippy --lib -- -D warnings` | exit 0 at end of plan |
| Format | `cargo fmt` | clean |
| Qualify (unchanged verdicts) | `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` | READY |
| Qualify | `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json` | PARTIAL (roles) |

## Scope

**In scope**:
- Re-seed fixture + harden the failing test with an actionable message
- Verify plan 024/026 done criteria against the live tree; flip statuses
  with evidence (or report gaps)
- Fix stale test counts and the DEVELOPER-GUIDE module map
- `cargo clippy --fix`, `cargo fmt`, manual dead-code removal
- New minimal CI workflow (fmt, clippy, test — including the seed step)
- Record the 2026-08-07 product direction in `plans/README.md` and
  refresh `CONTEXT.md`

**Out of scope**:
- Malloy removal (plan 027)
- Contoso intake (plan 023)
- Refactoring `route_request` or any behavior change
- Fixing clippy warnings that require design decisions (allow-list with a
  comment instead)

## Steps

### Step 1: Green baseline

```bash
cargo run --bin xmla_proxy -- seed-generated-db
cargo test --lib
```

Expected: seed prints non-zero counts; all tests pass.

Then harden
`execute::dispatch::tests::generated_project_fallback_measures_return_real_data`
(`src/execute/dispatch.rs:1514`): before opening the database, verify
`data/generated.db` exists AND contains the fact table (e.g. query
`duckdb_tables()` or attempt `SELECT 1 FROM dw_fys_f_undersökning LIMIT 0`).
On failure, panic with an actionable message:

```
data/generated.db is missing or empty — run:
    cargo run --bin xmla_proxy -- seed-generated-db
```

**Verify**: temporarily rename `data/generated.db`, run the single test,
confirm the message names the fix; restore by re-seeding.

### Step 2: Plan bookkeeping (024, 026)

Walk the done criteria of `plans/024-security-role-decision-gate.md` and
`plans/026-security-roles-and-user-context.md` against the live tree. For
each checklist item, record evidence as `file:line`. Pay attention to:

- 024: roles in `ProxyConfig`; converter emits roles into
  `proxy-config.json`; qualify reads roles from config (not markdown);
  `generated_project` verdict is PARTIAL with a clear role message.
- 026: `UserContext` type; trusted-header boundary with deny-closed 401;
  RLS predicates in emitted SQL; OLS table hiding; multi-role union
  semantics; role-filter E2E regression test; README "Security and roles"
  section accurate (including the Malloy-path caveat, which stays until
  plan 027 removes that path).

If every criterion is met: flip both rows in `plans/README.md` to DONE and
add a reconcile note (date, HEAD, test count, both qualify verdicts).
If any criterion is unmet: leave the row TODO, note the gap, and report —
do not mark DONE on vibes.

### Step 3: Doc drift fixes

- `README.md`: update test count (from "234 tests" to the current number
  after this plan lands).
- `CONTEXT.md`: update "Current state" date, test count, plans-completed
  line, and the "221 tests" line in What works today.
- `docs/DEVELOPER-GUIDE.md`: repair the module map —
  `execute/builders.rs` is now a thin shim; the execution path lives in
  `execute/runtime.rs`; kind handlers live in `execute/render.rs`
  (`dispatch_with_backend`); add `src/tools/` with its tools
  (`convert_tabular`, `data_loader`, `parse_tmdl`, `parse_bim`,
  `parse_folder`, `m_query`, `load_replay`, `trace_replay`, `qualify`,
  `inventory`, `extract_trace_mdx`, `seed_generated_db`, `seed_sql`,
  `tabular_model`); note `src/bin/*.rs` are thin wrappers over
  `tools::*::run()`; note `get_execute_statement_response` in
  `dispatch.rs` is test-only.

### Step 4: Lint bar

```bash
cargo clippy --fix --allow-dirty --allow-staged
cargo fmt
cargo test --lib   # must stay green
```

Then manually remove the never-used functions flagged by warnings
(verify zero callers with grep first; `axis_members.rs` and
`discover/members.rs` are the known hotspots). Finish with:

```bash
cargo clippy --lib -- -D warnings
```

If a warning class needs a design decision, `#[allow]` it locally with a
one-line justification comment instead of refactoring.

### Step 5: CI

Create `.github/workflows/ci.yml`:

- Trigger: push + pull_request.
- Job on `ubuntu-latest`, stable Rust toolchain, `Swatinem/rust-cache`.
- Steps: `cargo fmt --check` → `cargo clippy --lib -- -D warnings` →
  `cargo run --bin xmla_proxy -- seed-generated-db` → `cargo test --lib`.

The seed step is **required** — the generated_project execution test needs
`data/generated.db` and it is gitignored. Keep the workflow minimal; no
release automation in this plan.

### Step 6: Direction + index updates

Update `plans/README.md`:

- Add the "Product direction (locked 2026-08-07)" section if not already
  present.
- Add rows for 027 and 028; set this plan's row to DONE when finished.
- Execution order note: 028 → 027 → 023.

Update `CONTEXT.md` as a session checkpoint (goal, state, priorities,
backend direction per the 2026-08-07 decisions).

## Test plan

- `cargo test --lib` green after each step, not just at the end.
- Fresh-clone simulation: `rm data/generated.db`, then run only the CI
  sequence (seed → test) to prove the fixture self-heals.
- Qualify verdicts unchanged: retail READY, generated_project PARTIAL.

## Done criteria

- [ ] `cargo test --lib` -> 0 failures, including after deleting
      `data/generated.db` and following the CI sequence
- [ ] Failing-test message names the fix command
- [ ] Plans 024/026 statuses truthful, each with recorded file:line evidence
- [ ] `cargo clippy --lib -- -D warnings` exits 0; `cargo fmt --check` clean
- [ ] `.github/workflows/ci.yml` committed; includes the seed step
- [ ] README/CONTEXT/DEVELOPER-GUIDE counts and module map match `src/`
- [ ] `plans/README.md` updated (direction section, 027/028 rows, this row DONE)

## STOP conditions

- Plan 024/026 verification reveals an unimplemented criterion — report
  the gap instead of marking DONE.
- `cargo clippy --fix` breaks tests — revert the autofix and do the
  cleanup manually.
- The dead-code list contains a function that is actually called through
  a non-obvious path (e.g. from a test-only seam) — keep it and silence
  the warning with a comment instead.

## Maintenance notes

- The CI workflow is the point of this plan: every later plan (027, 023,
  Phase 4 epics) is gated by it automatically. Do not merge future work
  that fails it.
- Keep the seed tool as the single fixture-creation path; do not add a
  second way to build `data/generated.db`.
- Doc-drift fixes are one-time; the durable fix is updating docs in the
  same commit as the code change. State this expectation in
  `plans/README.md` when adding the direction section.
