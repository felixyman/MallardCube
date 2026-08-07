# Plan 027: Drop the Malloy runtime

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f3837cd..HEAD -- src/engine/ src/execute/ src/project/ src/main.rs src/tools/ js/ package.json Cargo.toml`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/028-hygiene-foundation.md` (green baseline + CI
  must exist before this excision lands)
- **Category**: direction
- **Planned at**: commit `f3837cd`, 2026-08-07

## Why this matters

Product direction locked 2026-08-07: **single runtime, direct SQL only.**
Malloy was an experiment in an optional semantic layer; `QueryPlan` turned
out to be the semantic layer. Today Malloy is:

- gated behind `MALLOY_RUNTIME=1`, never the default,
- **not covered by security-role enforcement** (admin-only on that path) —
  a standing caveat in the security docs,
- dependent on a long-lived Node.js worker (`js/`) — blocking the true
  single-binary story,
- a second emitter every future epic (hierarchies, DAX coverage, attached
  data sources) would have to feed and parity-test.

Removing it shrinks the maintenance surface, deletes the Node dependency,
makes role enforcement uniform on the only runtime, and lets the entry
point have an honest name. It must land **before** the Contoso intake
(023) so `generated_contoso/` is born Malloy-free.

## Current state (verified 2026-08-07)

Malloy-only modules (~1,135 lines):

- `src/engine/malloy.rs` (463) — Malloy source emitter
- `src/engine/malloy_compiler.rs` (69) — compiler trait
- `src/engine/malloy_node.rs` (140) — one-shot Node spike (legacy)
- `src/engine/malloy_node_longlived.rs` (259) — long-lived worker
- `src/engine/parity.rs` (204) — SQL-vs-Malloy parity tests

JS worker and npm files:

- `js/` — `malloy-worker.js`, `malloy-cli.js`, `malloy_rquickjs_entry.js`,
  `proxy-schema.js`, `package.json`, `package-lock.json`
- root `package.json` / `package-lock.json` — exists for `@malloydata/*`
  deps; verify nothing else uses it before deleting

Runtime integration:

- `src/execute/runtime.rs` (262) — `USE_MALLOY_RUNTIME` gate (:26),
  static `COMPILER` (:36), `warm_malloy_worker()` (:54), admin-only Malloy
  gate (:128-139), Malloy branch with PlanCache + direct-SQL fallback
  (:141-171). Entry point **named**
  `get_execute_cellset_response_timed_malloy_with_backend` (:110) — called
  by `main.rs`, `execute/builders.rs`, and dispatch tests.
- `src/main.rs` — `MALLOY_RUNTIME` env handling, warm-up call.

Config/loader coupling:

- `src/project/config.rs` — `ProxyConfig.malloy_model_file` (:110),
  `DimensionConfig.malloy_name` (:239), `MeasureConfig.malloy_name` (:262).
  **Verified**: the direct-SQL path never reads `malloy_name` (only the
  converter, `data_loader`, and the Malloy emitter do).
- `src/project/project.rs` — reads `model.malloy` from disk (:102-113,
  errors if missing), exposes `malloy_source()` (:118).

Converter/tooling references: `src/tools/convert_tabular.rs` (57),
`src/tools/data_loader.rs` (19), `src/tools/tabular_model.rs` (1).
`src/tools/qualify.rs` — check whether it validates `model.malloy`.

Support machinery: `src/engine/cache.rs` (`PlanCache`, serves the Malloy
compile path — verify no SQL-path user), `src/engine/timing.rs` (Malloy
timing fields).

Tests: `parity.rs` (11), emission tests in `malloy.rs` (14), `dispatch.rs`
references (12).

Docs: README Malloy rows + parity claims + "roles not enforced on Malloy
path" caveat; `docs/naming-contract.md` `malloy_name` rule;
`docs/DIAGRAMS.md`; `docs/DEVELOPER-GUIDE.md`; `CONTEXT.md`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Find Malloy refs | `grep -rni malloy src/ js/ docs/ README.md CONTEXT.md` | shrinking to zero (except deprecation notes) |
| Build | `cargo build` | exit 0 |
| Full tests | `cargo test --lib` | 0 failures |
| Qualify | `cargo run --bin xmla_proxy -- qualify generated_retail_analytics/proxy-config.json` | READY (unchanged) |
| Qualify | `cargo run --bin xmla_proxy -- qualify generated_project/proxy-config.json` | PARTIAL (unchanged) |
| Smoke | `cargo run` (serves project3) + execute one MDX via replay | valid cellset |

## Scope

**In scope**: everything listed in Current state; the tracing migration
(`eprintln!`/`debug_write` -> `tracing` crate + per-request IDs) because
this plan already touches `main.rs`/`runtime.rs` and deletes a third of
the log call sites.

**Out of scope**: hierarchies, converter behavior changes beyond stopping
`model.malloy` emission, multi-catalog, attached data sources.

## Steps

### Step 1: Delete the JS worker and npm files

`js/`, root `package.json`, `package-lock.json`. First verify root
`package.json` carries only Malloy-related deps/scripts (its test script
calls `cargo test --lib`; that moves to CI/CONTRIBUTING docs).

### Step 2: Delete Malloy engine modules

Remove `malloy.rs`, `malloy_compiler.rs`, `malloy_node.rs`,
`malloy_node_longlived.rs`, `parity.rs`; remove their `mod` declarations
from `src/engine/mod.rs`.

### Step 3: Collapse `execute/runtime.rs`

- Remove `USE_MALLOY_RUNTIME`, `enable/disable_malloy_runtime`,
  `COMPILER`, `malloy_compiler()`, `malloy_cache()`, `warm_malloy_worker()`,
  and the Malloy branch (including the admin-only gate and the fallback
  logic).
- Rename `get_execute_cellset_response_timed_malloy_with_backend` ->
  `get_execute_cellset_response_with_backend`.
- Update callers: `src/main.rs`, `src/execute/builders.rs`,
  `src/execute/dispatch.rs` tests.
- Keep the `_with_backend` injection seam and timing instrumentation for
  the SQL path.
- Keep the role-filter E2E regression test (`runtime.rs:223`) passing.

### Step 4: `main.rs`

Remove `MALLOY_RUNTIME` env handling and the warm-up call. Keep
`XMLA_TRACE`, `BIND_ADDRESS`, `PROXY_CONFIG`.

### Step 5: Config + loader decoupling

- `config.rs`: keep `malloy_model_file` / `malloy_name` **parseable** with
  `#[serde(default)]`; mark both deprecated in doc comments
  ("unused since plan 027; kept for config backward compatibility").
- `project.rs`: stop reading `model.malloy` from disk; remove
  `malloy_model_text` and `malloy_source()`. Existing project dirs
  (project2/3/4, generated_*) must load unchanged.
- Delete `model.malloy` files from sample/converted project dirs **after**
  the loader stops reading them; update README project-structure table.

### Step 6: Converter and tools

- `convert_tabular.rs` / `data_loader.rs` / `tabular_model.rs`: stop
  emitting `model.malloy` and stop populating `malloy_name` if trivially
  separable; if `malloy_name` removal ripples through shared types, leave
  the field emitted-but-unused (deprecated) — do not gold-plate.
- `qualify.rs`: remove any `model.malloy` existence/parse validation.
- `conversion-report.md` template: drop the Malloy section.

### Step 7: Trim support machinery

- `cache.rs`: if `PlanCache` has no remaining caller after Step 3, delete
  the module; otherwise keep the used part.
- `timing.rs`: drop Malloy-only fields; keep SQL timings.

### Step 8: Tracing migration

- Add `tracing` + `tracing-subscriber` (fmt layer; env filter via
  `RUST_LOG`; keep a file appender if the debug-log file behavior is
  worth preserving — prefer `tracing-appender` or document the change).
- Replace `eprintln!` and `debug_write` call sites with structured spans/
  events. Generate a request ID (existing `uuid` dep) in `handle_xmla`
  and span the request lifecycle with it.
- Keep tower-http `TraceLayer` for HTTP spans.
- Document `RUST_LOG` in README/DEVELOPER-GUIDE; state the debug-log
  behavior change explicitly if the file goes away.

### Step 9: Tests

- Delete parity and Malloy-emission tests.
- Fix remaining compile errors (dispatch/project tests referencing
  `malloy_source`, `malloy_name` construction in fixtures — keep the field
  with `..Default::default()` where possible).
- The SQL-path oracle tests (raw-DuckDB assertions, excel_trace_*) are
  the regression net now. Do not weaken them.

### Step 10: Docs

- README: remove Malloy runtime section/rows and the parity claim; remove
  the "roles not enforced on Malloy path" row (roles become uniformly
  enforced — update the "What is enforced" table); update project
  structure table (no `model.malloy`); single-runtime architecture
  statement.
- `docs/naming-contract.md`: `malloy_name` deprecated rule.
- `docs/DEVELOPER-GUIDE.md`: lifecycle + env vars (`MALLOY_RUNTIME` gone,
  `RUST_LOG` in).
- `docs/DIAGRAMS.md`: drop the Malloy path from diagrams.
- `CONTEXT.md`: key-files table + constraints.

## Test plan

- `cargo test --lib` green at every step boundary.
- Qualify verdicts unchanged for both converted projects.
- Manual smoke: serve `project3`, run `trace-replay` against an existing
  trace (or capture a fresh one pre-removal and replay post-removal).

## Done criteria

- [ ] `grep -rni malloy src/ docs/ README.md CONTEXT.md` returns only
      deprecation comments and historical plan files
- [ ] No `js/`, no npm files, no Node dependency; `cargo build` produces
      the standalone binary
- [ ] `cargo test --lib` exits 0 (CI green)
- [ ] Qualify: retail READY, generated_project PARTIAL (unchanged)
- [ ] `tracing` with request IDs in place; `RUST_LOG` documented
- [ ] Existing project configs load unchanged (backward compatibility)
- [ ] Docs updated; `plans/README.md` row -> DONE

## STOP conditions

- A non-test caller of `malloy_source()` / `PlanCache` surfaces that the
  direct-SQL path actually depends on — reassess trim scope before
  deleting.
- `qualify`/`trace_replay`/`load_replay` tooling proves to require
  `model.malloy` to function — decouple first, then continue.
- Config backward compatibility breaks (an existing project dir fails to
  load) — restore compat before proceeding; compat is contractual.

## Maintenance notes

- Config compatibility is contractual: old project directories must keep
  loading. Deprecated fields stay parseable indefinitely.
- The trace-replay + oracle tests are now the only cross-check on the SQL
  emitter. Treat new Excel behaviors as failing replay tests first.
- Deleting the "roles not enforced on Malloy path" caveat from the
  security docs is part of the security story: after this plan, roles are
  enforced on the only runtime there is.
