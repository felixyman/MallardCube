# Plan 020: Reconcile the public CLI and documentation contract

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- README.md docs/DEVELOPER-GUIDE.md CONTEXT.md package.json generated_project/conversion-report.md generated_project/bootstrap.sql generated_retail_analytics/conversion-report.md generated_retail_analytics/bootstrap.sql src/main.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: `plans/016-placeholder-sql-contract.md`, `plans/017-config-relative-db-paths.md`, `plans/018-qualify-trace-init-flow.md`
- **Category**: DX / docs
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The repo now has a stronger single-binary story and a qualification gate, but
the public docs still mix old wrapper-binary commands, stale test counts, and
inconsistent runtime-path descriptions.

That makes the repo harder to trust exactly when it is trying to become a
repeatable migration tool.

## Current state

Relevant excerpts:

```md
// README.md:1-9
Malloy is the primary semantic path. Direct SQL is the automatic fallback...
```

```rust
// src/main.rs
Malloy runtime only enables when MALLOY_RUNTIME=1; Direct SQL is default.
```

```md
// docs/DEVELOPER-GUIDE.md
221 tests
cargo run --bin convert_tabular -- ...
cargo run --bin trace_replay ...
```

```json
// package.json:9-10
"test": "echo \"Error: no test specified\" && exit 1"
```

```md
// generated_retail_analytics/conversion-report.md
bootstrap ... sets db_path in proxy-config.json
```

```sql
-- generated_retail_analytics/bootstrap.sql
.read schema.sql
.read seed_date_dim.sql
```

Repo conventions to match:

- One public CLI contract is better than multiple half-deprecated entrypoints.
- Docs should describe the actual default runtime path.
- Generated reports should not claim a bootstrap script edits JSON unless it really does.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Full tests | `cargo test --lib` | all pass |
| CLI help | `cargo run --bin xmla_proxy -- --help` | shows supported subcommands |
| npm test | `npm test` | no longer guaranteed failure |

## Scope

**In scope**:
- `README.md`
- `docs/DEVELOPER-GUIDE.md`
- `CONTEXT.md`
- `package.json`
- generated project conversion reports / bootstrap comments

**Out of scope**:
- major code behavior changes not already landed in earlier plans

## Steps

### Step 1: Pick one CLI contract and one runtime-path description

Use the single `xmla_proxy` binary with subcommands as the public interface.
Document wrapper binaries only as compatibility shims if they remain.

Describe runtime as:

- Direct SQL by default
- Malloy opt-in with `MALLOY_RUNTIME=1`

**Verify**: `cargo run --bin xmla_proxy -- --help` -> matches the documented commands.

### Step 2: Remove stale counts and contradictory claims

Update docs so they no longer hard-code obsolete test counts or completed-plan
ranges. Point volatile status claims at `plans/README.md` when possible.

**Verify**: grep docs for `221 tests`, `228 tests`, and `Plans 001–010` -> no stale claims remain.

### Step 3: Make bootstrap text match actual behavior

Update generated report/bootstrap wording so it matches plan 017’s final path
contract and does not claim bootstrap mutates `proxy-config.json` unless code
actually does so.

### Step 4: Replace the placeholder npm test script

Point `npm test` at a supported verification path or annotate/remove the
guaranteed-failure placeholder.

**Verify**: `npm test` -> exits 0 or gives a truthful supported verification path.

## Test plan

- Docs-only grep checks are sufficient if no code behavior changes are introduced.
- Re-run `cargo test --lib` after doc/script updates.

## Done criteria

- [ ] `cargo test --lib` exits 0
- [ ] Public docs consistently use the single-binary CLI contract
- [ ] Docs consistently describe Direct SQL as default and Malloy as opt-in
- [ ] Bootstrap docs/comments match actual `db_path` behavior
- [ ] `npm test` is no longer a guaranteed false negative
- [ ] `plans/README.md` status row updated

## STOP conditions

- Behavior remains unresolved in plans 016–018, making a stable public contract impossible.

## Maintenance notes

- Keep volatile counts out of multiple docs where possible.
- If wrapper binaries remain, label them clearly as compatibility wrappers.
