# Plan 011: Add a converted-project qualification command

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/main.rs src/tools/mod.rs src/tools/inventory.rs src/tools/trace_replay.rs src/project/project.rs README.md generated_project/conversion-report.md generated_retail_analytics/conversion-report.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: direction
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The next product milestone is not more demo coverage. It is proving three real
customer-shaped Tabular models end-to-end with a repeatable intake loop.

The repo now has all the raw pieces: a single-binary CLI, a converter, an
inventory tool, and a compatibility gate. What is still missing is a single,
operator-friendly readiness verdict that says whether a converted project is
actually ready for Excel, partially ready, or blocked and why.

This plan creates that qualification command so the next model proofs are run
through one consistent gate instead of ad hoc human inspection.

## Current state

- `src/main.rs` — the single binary now exposes separate `serve`, `convert-tabular`, `trace-replay`, `extract-trace`, `inventory`, `seed-generated-db`, and `seed-sql` commands.
- `src/tools/inventory.rs` — can inspect a Tabular Editor export and count simple/sql_fallback/manual measures, date-role tables, relationships, and roles, but only for source exports.
- `src/tools/trace_replay.rs` — can validate discover + execute traces for a converted project, but only when a trace already exists.
- `generated_project/conversion-report.md` — shows unsupported roles and manual loading requirements, but there is no machine-readable readiness verdict.
- `generated_retail_analytics/conversion-report.md` — shows a small retail model still requiring operator judgment even though the runtime already supports more than the report communicates.

Relevant excerpts:

```rust
// src/main.rs:25-60
#[derive(Subcommand)]
enum Command {
    Serve,
    ConvertTabular { src_dir: String, out_dir: String },
    TraceReplay { trace_path: String, project: Option<String> },
    ExtractTrace { path: String },
    Inventory { src_dir: String },
    SeedGeneratedDb,
    SeedSql,
}
```

```rust
// src/tools/inventory.rs:128-157
for m in &t.measures {
    match m.classification.as_str() {
        "simple" => simple += 1,
        "sql_fallback" => sql_fallback += 1,
        "manual" => manual += 1,
        _ => {}
    }
}
```

```rust
// src/tools/trace_replay.rs:21-47,156-176
pub fn run(args: Vec<String>) -> i32 {
    let trace_path = args.iter().find(|a| a.ends_with(".jsonl")).unwrap_or(...);
    crate::proxy_project::init_project(config_path).expect("init project");
    crate::backend::init_backend(p.config.db_path.as_deref()).expect("init backend");
    ...
    if total_failed > 0 {
        return 1;
    }
    0
}
```

```md
// generated_project/conversion-report.md:76-112
All tables use M (Power Query) partitions and must be loaded into DuckDB manually.
...
Security roles detected but NOT supported by the proxy.
```

Repo conventions to match:

- New operator tooling now lives under `src/tools/` and is wired into `src/main.rs` subcommands.
- Excel safety claims are backed by explicit tests or the compatibility gate, not by comments.
- Fail closed on unsupported semantics; a qualification report should surface blockers instead of hiding them.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| CLI help | `cargo run --bin xmla_proxy -- --help` | exits 0 and lists the new qualification command |
| Full tests | `cargo test --lib` | all pass |
| Retail qualification | `cargo run --bin xmla_proxy -- <new-command> generated_retail_analytics/proxy-config.json` | exit 0 and emits a readiness verdict |
| Healthcare qualification | `cargo run --bin xmla_proxy -- <new-command> generated_project/proxy-config.json` | exit 0 and emits a readiness verdict |

## Scope

**In scope**:
- `src/main.rs`
- `src/tools/mod.rs`
- one new tool module under `src/tools/` for qualification
- `src/project/project.rs` only if a small helper is needed to inspect project/model state cleanly
- qualification-focused tests under an existing Rust test file
- `README.md`

**Out of scope**:
- changing measure semantics or converter translation logic
- redesigning the compatibility gate itself
- role enforcement implementation
- hierarchy/TMSCHEMA expansion

## Steps

### Step 1: Define the qualification contract

Introduce a narrow readiness model for converted projects, derived from machine-
readable state rather than markdown parsing. At minimum, support:

- `READY` — can load, has no known blockers, and optional replay passed
- `PARTIAL` — usable but requires manual follow-up (for example, unsupported roles or manual-only measures)
- `BLOCKED` — cannot honestly be claimed Excel-safe (for example, stub fallback files, missing load path, broken config)

Reason codes should come from real project/config/fallback state, not from free-
text `conversion-report.md` parsing.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Implement a qualification command in the single binary

Add a new CLI subcommand that accepts a converted `proxy-config.json` path and
optionally a trace path. It should:

- load the project
- inspect dimensions, measures, fallback files, date-role metadata, and `db_path`
- detect unsupported roles when a sibling conversion artifact exposes them
- optionally run the existing replay gate when a trace is supplied
- print a concise readiness summary plus machine-readable reason codes

Do not shell out to separate binaries if a small library helper can keep the
logic in-process and testable.

**Verify**: `cargo run --bin xmla_proxy -- --help` -> shows the new command.

### Step 3: Add qualification tests for both checked-in converted projects

Add tests that prove:

- `generated_retail_analytics` is qualified consistently with its current runtime contract
- `generated_project` is not overclaimed when roles, manual loading, or stubbed fallbacks still exist
- stub fallback files and missing runtime assets produce stable reason codes

Model these tests after the current `src/project/project.rs` and replay-oriented
regression style already used in the repo.

**Verify**: `cargo test --lib` -> all pass.

### Step 4: Document the intake loop

Add a short README workflow for real-model intake using the single binary:

1. inventory/export inspection
2. conversion
3. qualification
4. optional trace replay after Excel capture

Keep this operator-focused and specific to the Excel-first migration goal.

**Verify**: `grep -n "qualif" README.md` -> shows the new workflow text.

## Test plan

- Add qualification tests for `generated_retail_analytics` and `generated_project`.
- Cover at least: good load, stub fallback detection, optional replay passthrough, and unsupported-role signaling.
- Follow the structure of existing `src/project/project.rs` tests for project loading and the current `trace_replay` verification style.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] The single binary exposes a qualification command for converted projects
- [ ] The qualification result is derived from machine-readable repo artifacts, not only markdown parsing
- [ ] Both checked-in converted projects have qualification coverage
- [ ] `plans/README.md` status row updated

## STOP conditions

- The necessary readiness facts are not derivable from config/model/fallback files without brittle parsing of prose reports.
- Reusing the existing replay logic requires a large refactor outside the in-scope files.
- The proposed verdict model cannot distinguish “partial but usable” from “blocked” honestly with the current runtime signals.

## Maintenance notes

- Reviewers should check that the qualification command is conservative; false confidence is worse than a red flag.
- Keep the verdict model stable and machine-readable so later plans can use it as a gate.
- Do not let this plan become a generic deployment/ops system; it is only the migration qualification loop.
