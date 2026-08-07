# Plan 017: Resolve `db_path` relative to the config file everywhere

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat c89764f..HEAD -- src/main.rs src/tools/qualify.rs src/tools/trace_replay.rs src/project/project.rs generated_retail_analytics/proxy-config.json generated_project/proxy-config.json README.md docs/DEVELOPER-GUIDE.md`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: —
- **Category**: correctness
- **Planned at**: commit `c89764f`, 2026-06-17

## Why this matters

Generated projects now emit `db_path` values intended to live beside
`proxy-config.json`, but the server, replay tool, and qualify flow do not all
interpret that path the same way.

That means a project can qualify against one database and serve against another.

## Current state

- Converter output for date-role projects uses `"db_path": "data/<cube>.db"`.
- `qualify()` resolves that path relative to the config directory.
- `serve` and `trace_replay` use `db_path` verbatim from process CWD.

Relevant excerpts:

```json
// generated_retail_analytics/proxy-config.json:1-8
"db_path": "data/sales.db"
```

```rust
// src/tools/qualify.rs:67-76
let resolved = Path::new(config_path).parent().map(|d| d.join(db))...
```

```rust
// src/main.rs:158-162
if let Some(path) = p.config.db_path.as_deref() {
    backend::init_backend(Some(path))
}
```

```rust
// src/tools/trace_replay.rs:35-37
crate::backend::init_backend(p.config.db_path.as_deref()).expect("init backend");
```

Repo conventions to match:

- Project-local artifacts should be runnable from their own checked-in config.
- The same config field must mean the same thing in `serve`, `qualify`, and
  `trace_replay`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build CLI | `cargo build --bin xmla_proxy` | exit 0 |
| Qualify retail | `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` | deterministic verdict |
| Replay with project | `cargo run --bin xmla_proxy -- trace-replay xmla-trace.jsonl generated_project/proxy-config.json` | deterministic path resolution |
| Full tests | `cargo test --lib` | all pass |

## Scope

**In scope**:
- `src/main.rs`
- `src/tools/qualify.rs`
- `src/tools/trace_replay.rs`
- one shared config-path resolution helper if needed
- config/path tests

**Out of scope**:
- converter contract changes beyond consuming the existing `db_path`
- docs-only cleanup without behavior change

## Steps

### Step 1: Introduce one shared `db_path` resolution rule

Add one canonical helper that resolves a configured `db_path` against the
directory containing `proxy-config.json`. Use it everywhere instead of repeating
ad hoc logic.

Prefer locating this near project/config loading so all callers can share it.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 2: Apply the rule to serve, qualify, and replay

Update the three call sites so they all open the same database for the same
config file:

- server startup in `src/main.rs`
- qualification in `src/tools/qualify.rs`
- replay in `src/tools/trace_replay.rs`

Keep the behavior for absolute paths explicit and unchanged.

**Verify**: add tests that the resolved path for `generated_retail_analytics/proxy-config.json` is `generated_retail_analytics/data/sales.db` and that `generated_project/proxy-config.json` resolves to `data/generated.db`.

### Step 3: Lock the behavior with tests

Add focused tests around relative and absolute path handling so this contract
does not drift again.

**Verify**: `cargo test --lib` -> all pass.

## Test plan

- Unit tests for config-relative path resolution.
- At least one integration-style test covering a converted project in a
  subdirectory.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib` exits 0
- [ ] `serve`, `qualify`, and `trace_replay` all resolve `db_path` the same way
- [ ] Generated project configs point at the same physical DB in every tool path
- [ ] `plans/README.md` status row updated

## STOP conditions

- The current loader structure makes shared config-path resolution impossible
  without a large refactor.
- There are hidden callers relying on the old CWD-relative interpretation.

## Maintenance notes

- This plan should make path behavior boring and invisible.
- Once the rule is centralized, docs can safely describe one contract in plan 020.
