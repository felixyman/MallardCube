# Plan 018: Remove the `qualify <config> <trace>` singleton-init crash

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/tools/qualify.rs src/tools/trace_replay.rs src/project/project.rs src/main.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: `plans/017-config-relative-db-paths.md`
- **Category**: correctness
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

The replay-gated qualification path is supposed to be the highest-confidence
converted-project check. Right now it can panic because `qualify()` touches the
global project singleton before `trace_replay` tries to initialize the target
project.

## Current state

Relevant excerpts:

```rust
// src/tools/qualify.rs:57-62
if crate::proxy_project::project().config.catalog != p.config.catalog {
    let _ = crate::proxy_project::init_project(Some(config_path));
}
```

```rust
// src/project/project.rs
pub fn project() -> &'static ProxyProject {
    PROJECT.get_or_init(|| ProxyProject::load("project3/proxy-config.json").expect(...))
}
```

```rust
// src/tools/trace_replay.rs:30-32
crate::proxy_project::init_project(config_path).expect("init project");
```

Repo conventions to match:

- CLI tooling should fail with a verdict, not a singleton panic.
- Prefer explicit project handoff over hidden global initialization.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Qualify with trace | `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json xmla-trace.jsonl` | exits with verdict, no panic |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/tools/qualify.rs`
- `src/tools/trace_replay.rs`
- tiny shared/project helper changes only if needed
- CLI/tool tests for the trace-qualified path

**Out of scope**:
- global singleton removal across the repo
- replay diff logic changes unrelated to init flow

## Steps

### Step 1: Make `qualify()` stop touching the global project prematurely

Refactor the qualification path so it does not call `project()` before deciding
how replay will run.

The two acceptable shapes are:

- `qualify()` passes a loaded project into replay without re-init, or
- `qualify()` defers all singleton access until after replay setup is complete.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Add one regression test for the trace path

Add a test or narrowly scoped CLI regression that exercises the equivalent of:

`qualify <config> <trace>`

and proves it returns a verdict instead of panicking.

**Verify**: `cargo test --lib qualify` or the new targeted test filter -> pass.

## Test plan

- One regression for qualification with a trace argument.
- Keep the test focused on init/order behavior, not replay contents.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] `qualify <config> <trace>` no longer panics because of singleton init order
- [ ] The fix stays scoped to CLI/tooling, not a repo-wide singleton rewrite
- [ ] `plans/README.md` status row updated

## STOP conditions

- The only safe fix requires removing the global project singleton everywhere.
- Replay currently depends on hidden singleton side effects that cannot be isolated cleanly.

## Maintenance notes

- Keep the fix surgical.
- This plan should compose cleanly with plan 017’s shared `db_path` handling.
