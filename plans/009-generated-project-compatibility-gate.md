# Plan 009: Raise generated_project to an Excel compatibility gate

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat a1b1bd4..HEAD -- src/bin/trace_replay.rs src/xmla_trace.rs src/execute/dispatch.rs src/test_support/fixtures.rs src/project/project.rs generated_project/ data/seed_generated.sql src/bin/seed_generated_db.rs README.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: LOW
- **Depends on**: `plans/008-converter-time-metadata.md`
- **Category**: direction
- **Planned at**: commit `a1b1bd4`, 2026-06-16

## Why this matters

The repo's strongest Excel proof today is still centered on `project3` and on
execute-only replay. That is valuable, but it does not yet prove that a
converted customer-shaped model survives the full discover + execute loop.

This plan turns `generated_project` from a loadable artifact into a standing
compatibility gate. That gives future time-intelligence and converted-measure
work a realistic regression target instead of relying only on the demo cube.

## Current state

- `src/bin/trace_replay.rs` — replays only `ExecuteStatement` entries.
- `src/xmla_trace.rs` — already captures all XMLA requests and responses.
- `src/execute/dispatch.rs` — replay/oracle tests are concentrated on project3.
- `src/project/project.rs` — generated-project tests are still mainly load/smoke assertions.
- `generated_project/` and `data/generated.db` — now exist and boot successfully.

Relevant excerpts:

```rust
// src/bin/trace_replay.rs:59-62
let kind = v.get("request_kind").and_then(|k| k.as_str()).unwrap_or("");
if kind != "ExecuteStatement" {
    continue;
}
```

```rust
// src/xmla_trace.rs:41-72
// trace mode writes full request_xml, response_xml, mdx, and timings
```

```rust
// src/execute/dispatch.rs: project3 replay tests dominate current coverage
// src/project/project.rs: generated_project tests are still smoke/load structure
```

Repo conventions to match:

- Replay tooling lives in `src/bin/`.
- Fixture-derived regression tests belong in `src/execute/dispatch.rs` and `src/test_support/fixtures.rs`.
- Keep generated-project data small and deterministic.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build replay tool | `cargo build --bin trace_replay` | exit 0 |
| Seed generated DB | `cargo run --bin seed_generated_db` | exit 0 |
| Replay trace | `cargo run --bin trace_replay -- xmla-trace.jsonl --project generated_project/proxy-config.json` | exit 0 after this plan |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/bin/trace_replay.rs`
- `src/xmla_trace.rs`
- `src/execute/dispatch.rs`
- `src/test_support/fixtures.rs`
- `src/project/project.rs`
- `src/bin/seed_generated_db.rs`
- `data/seed_generated.sql`
- files under `generated_project/` needed for replay fixtures/tests
- `README.md`

**Out of scope**:
- redesigning query execution semantics for complex fallback measures (Plan 010)
- adding new converter features beyond what Plan 008 already emitted
- Power BI / TMSCHEMA coverage

## Steps

### Step 1: Extend replay tooling beyond execute-only

Teach the replay harness to optionally validate discover/metadata traffic as
well as execute traffic. Use the already captured `request_kind`,
`request_xml`, and `response_xml` fields in `xmla-trace.jsonl`.

The goal is not perfect byte-for-byte SOAP matching. The goal is a stable,
structural Excel gate for the rowsets and cellsets Excel actually uses.

**Verify**: `cargo build --bin trace_replay` -> exit 0.

### Step 2: Capture and check in a minimal generated-project Excel session

Create one small but representative trace fixture against `generated_project`:

- initial discover handshake
- one simple measure
- one relationship-backed dimension filter
- one supported time-aware measure, if Plan 008 made one available

Do not capture sensitive/customer data; use the synthetic generated DB.

**Verify**: replay the fixture locally and get zero diffs.

### Step 3: Add generated-project replay tests to the library suite

Check the important generated-project MDX shapes into `src/test_support/fixtures.rs`
and add tests to `src/execute/dispatch.rs` / `src/project/project.rs` that prove:

- discover rowsets render successfully
- execute responses are structurally stable
- relationship-backed dimensions and emitted time metadata survive the full path

**Verify**: `cargo test --lib generated_project` -> pass.

### Step 4: Document the compatibility gate workflow

Update the README so the workflow is discoverable:

- seed generated DB
- run proxy on generated_project
- capture trace
- replay trace

Keep this short and operator-focused.

**Verify**: `grep -n "trace_replay" README.md` shows the new workflow.

## Test plan

- New trace-replay unit coverage for discover rowsets.
- Generated-project replay tests added to the library suite.
- One full replay command exercised against synthetic generated data.

## Done criteria

- [ ] `cargo build --bin trace_replay` exits 0
- [ ] `cargo run --bin trace_replay -- xmla-trace.jsonl --project generated_project/proxy-config.json` exits 0 on the checked-in generated-project fixture
- [ ] `cargo test --lib` exits 0
- [ ] The repo has a checked-in generated-project replay path, not just smoke/load tests
- [ ] `plans/README.md` status row updated

## STOP conditions

- Generated-project replay requires unsupported complex fallback measures to be correct before any fixture can pass.
- The synthetic generated DB cannot produce stable enough values for deterministic replay.
- Discover responses vary in a non-deterministic way that makes structural replay meaningless.

## Maintenance notes

- This plan is about proof, not new semantics. Keep it narrow.
- Reviewers should check that replay assertions are structural and stable, not brittle byte-for-byte XML comparisons.
