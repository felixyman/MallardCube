# Plan 025: Make direct-SQL XMLA execution concurrent across users

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f9464d0..HEAD -- src/main.rs src/backend/mod.rs src/engine/plan.rs src/execute/builders.rs src/execute/runtime.rs src/execute/dispatch.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: none
- **Category**: perf / tech-debt
- **Planned at**: commit `f9464d0`, 2026-06-21

## Why this matters

The proxy can accept multiple TCP connections today, but its query execution is
effectively single-file because all requests share one global `DuckDB`
connection behind one `Mutex`. Under real Excel usage that means one active
query blocks every other user, even when they are reading the same model.

The goal of this plan is to make the default direct-SQL runtime path genuinely
multi-user: each request should be able to obtain its own backend handle,
execute without contending on a process-wide connection mutex, and do that work
off the async server thread. This keeps the current product constraint intact:
`QueryPlan -> SQL -> DuckDB` remains the safe default runtime path.

## Current state

Relevant files and their roles:

- `src/backend/mod.rs` — owns DuckDB connection creation and the global backend
  singleton.
- `src/main.rs` — server startup, `/xmla` route, and request dispatch.
- `src/engine/plan.rs` — convenience execution wrappers that currently pull the
  global backend.
- `src/execute/builders.rs` — public execute helpers; already contains one
  explicit backend-injection path.
- `src/execute/runtime.rs` — timed execute path used by `main.rs`; still pulls
  global project/backend state for the live server path.
- `src/execute/dispatch.rs` — tests; already contains a test-only injected
  backend implementation that proves the codebase accepts non-singleton
  backends.
- `README.md` and `CONTEXT.md` — document that direct SQL is the default safe
  runtime path and Malloy is optional.

The current backend shape serializes all live execution:

```rust
// src/backend/mod.rs:3-7
use std::sync::{Mutex, OnceLock};

pub struct Backend {
    conn: Mutex<Connection>,
}
```

```rust
// src/backend/mod.rs:214-231
static BACKEND: OnceLock<Backend> = OnceLock::new();

pub fn init_backend(db_path: Option<&str>) -> Result<(), duckdb::Error> {
    let backend = match db_path {
        Some(path) => Backend::open(Path::new(path))?,
        None => Backend::new()?,
    };
    BACKEND.set(backend).map_err(|_| {
        duckdb::Error::InvalidParameterName("Backend already initialised".into())
    })?;
    Ok(())
}
```

Production execution wrappers still reach for that singleton implicitly:

```rust
// src/engine/plan.rs:222-230
pub fn execute_plan(plan: &QueryPlan, model: &SemanticModel) -> QueryResult {
    execute_plan_with_backend(plan, model, Backend::get())
}

pub fn execute_plan_with_sql(plan: &QueryPlan, sql: &str) -> QueryResult {
    execute_plan_sql_with_backend(plan, sql, Backend::get())
}
```

Server startup initializes the global project and backend once, and the async
request handler performs all work inline:

```rust
// src/main.rs:147-175
let config_path = std::env::var("PROXY_CONFIG")
    .ok()
    .unwrap_or_else(|| "project3/proxy-config.json".into());
proxy_project::init_project(Some(&config_path))
    .expect("init project");

let db_path = proxy_project::resolve_db_path(
    &config_path,
    p.config.db_path.as_deref(),
);
match db_path {
    Some(path) => backend::init_backend(Some(&path))
        .expect(&format!("failed to open DuckDB: {path}")),
    None => backend::init_backend(None)
        .expect("failed to init demo DuckDB"),
}
```

```rust
// src/main.rs:249-270
async fn handle_xmla(body: String) -> impl IntoResponse {
    let headers = default_headers();
    let request = parse_xmla(&body);
    let response_body = route_request(&request, &body);
    (StatusCode::OK, headers, response_body)
}
```

There is already an injection pattern worth reusing instead of adding more
globals:

```rust
// src/execute/builders.rs:37-58
pub fn execute_semantic_query_with_backend<B: QueryBackend>(
    query: &SemanticQuery,
    backend: &B,
    model: &SemanticModel,
) -> String {
    let plan = plan_from_semantic(query);
    let result = execute_plan_with_backend(&plan, model, backend);
    dispatch(query, &result)
}

pub fn get_execute_cellset_response_with_backend<B: QueryBackend>(
    mdx: &str,
    backend: &B,
    model: &SemanticModel,
) -> String {
    let query = crate::mdx_semantic::semantic_query_from_mdx(mdx);
    execute_semantic_query_with_backend(&query, backend, model)
}
```

```rust
// src/execute/dispatch.rs:152-156
/// Test-only `QueryBackend` that wraps a file-based DuckDB connection.
/// Avoids the global `Backend` singleton so converted-project tests can
/// exercise their own databases without in-memory demo seeding.
struct FileQueryBackend(std::sync::Mutex<duckdb::Connection>);
```

Two documented constraints must remain true after the refactor:

- `README.md:8-9` — "Direct SQL is the default runtime path. Malloy is
  optional (`MALLOY_RUNTIME=1`) and verified by parity tests."
- `CONTEXT.md:73-77` — prefer correctness over guessing, fail closed on
  unsupported semantics, and keep `QueryPlan -> SQL -> DuckDB` as the default.

Repo conventions to match:

- Tests live inline in `#[cfg(test)]` modules; see `docs/DEVELOPER-GUIDE.md:224-240`.
- New server-path code should prefer explicit dependency passing over adding new
  globals; `get_execute_cellset_response_with_backend()` and the test-only
  `FileQueryBackend` are the existing exemplar.
- Keep the change minimal: do not redesign the converter, semantic model, or
  Malloy compiler protocol as part of this plan.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Build binary | `cargo build --bin xmla_proxy` | exit 0 |
| Targeted concurrency tests | `cargo test --lib concurrent_` | all new concurrency tests pass |
| Full library tests | `cargo test --lib` | all tests pass |

## Scope

**In scope**:

- `src/backend/mod.rs`
- `src/main.rs`
- `src/engine/plan.rs`
- `src/execute/builders.rs`
- `src/execute/runtime.rs`
- `src/execute/dispatch.rs` tests

**Out of scope**:

- `src/project/project.rs` global project singleton removal
- `js/malloy-worker.js` or Malloy worker pooling
- XMLA authentication/security roles
- converter output, generated project assets, or qualification logic
- full SSAS-style per-user session state; a fixed shared `SessionId` is a
  follow-up concern, not part of this execution-path change

## Git workflow

- Branch: `advisor/025-concurrent-execution`
- Commit message style in recent history is short plain phrases (examples:
  `progress correctness`, `single-binary refactor`). Use a concise imperative
  phrase that matches that simplicity.
- Do NOT push or open a PR unless the operator explicitly asks for it.

## Steps

### Step 1: Add characterization tests before removing the singleton path

Add tests that prove the intended concurrency contract, using the inline-test
pattern from `src/execute/dispatch.rs`.

Add at least these tests:

1. A backend-level test in `src/backend/mod.rs` that opens the same file-backed
   DuckDB database through two independent backend handles and verifies both can
   read successfully from parallel threads.
2. A demo-backend test in `src/backend/mod.rs` that checks two independently
   checked-out demo backends return the same seeded scalar result. This guards
   against accidentally replacing the current shared demo dataset with
   per-request isolated in-memory databases.
3. A server-path or execute-path test in `src/execute/dispatch.rs` whose test
   names start with `concurrent_`, launches multiple threads against injected
   backends, and verifies all requests return valid cellsets.

Do not use flaky wall-clock thresholds as the only proof. The tests should
prove that the code path supports multiple independent backend handles and that
the returned XML is correct.

**Verify**: `cargo test --lib concurrent_` -> all new concurrency tests pass.

### Step 2: Replace the production backend singleton with a request-checkout source

Refactor `src/backend/mod.rs` so production code no longer depends on one
process-wide `OnceLock<Backend>` holding one `Mutex<Connection>`.

Target code shape:

- Keep `Backend` as the `QueryBackend` implementation for one connection.
- Introduce one minimal new type in `src/backend/mod.rs` that owns the runtime
  database configuration and can produce independent `Backend` instances for
  requests. Call it something explicit like `BackendSource` or
  `BackendFactory`; do not scatter this logic across `main.rs` and runtime
  modules.
- For file-backed projects, each checkout should open its own `Connection` to
  the same DuckDB file.
- For the demo path (`db_path == None`), seed one shared database artifact once
  and open request backends against that shared artifact. Do **not** use
  `Connection::open_in_memory()` per request, because that would create one
  separate demo database per user.
- Preserve existing benchmark constructors such as `Backend::new_with_config()`.

The simplest acceptable demo implementation is to extract the current demo
seeding logic into a helper that can populate a file-backed connection, then
create a temporary/shared file for the demo source at startup.

Do not try to build a sophisticated connection pool unless the crate API makes
simple checkout impossible. The minimal goal is independent read connections,
not a full pooling subsystem.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 3: Thread the backend explicitly through the live execute path

Refactor the production execute path so it no longer reaches `Backend::get()`.

Required changes:

1. In `src/engine/plan.rs`, keep `execute_plan_with_backend()` and
   `execute_plan_sql_with_backend()` as the canonical execution helpers.
2. Update `src/execute/builders.rs` and `src/execute/runtime.rs` so the code
   used by the live server path accepts `&dyn QueryBackend` or `&impl QueryBackend`
   explicitly for both direct-SQL execution and the Malloy-compiled-SQL path.
3. Any convenience wrappers that still use `Backend::get()` must become
   bench/test-only conveniences; they must not be reachable from `main.rs`
   request handling after this refactor.

Follow the existing `get_execute_cellset_response_with_backend()` pattern rather
than inventing another global state access path.

`MALLOY_RUNTIME=1` may still serialize compile requests through the single
worker. That is acceptable for this plan. The important part is that once SQL
is ready, execution uses the per-request backend checkout rather than the old
global backend.

**Verify**: `cargo test --lib concurrent_` -> still passes.

### Step 4: Move blocking request execution off the async runtime thread

Refactor `src/main.rs` to carry shared server state explicitly and run blocking
work inside `tokio::task::spawn_blocking`.

Target shape:

- Add a minimal `AppState` in `src/main.rs` holding the request-checkout backend
  source (and any other server-only immutable state you need).
- Wire Axum `State<Arc<AppState>>` into the `/xmla` route.
- Keep lightweight request parsing/logging in the async handler if convenient,
  but move `route_request()` execution into `spawn_blocking` so DuckDB work,
  plan generation, and XML rendering do not occupy the async reactor thread.
- Within the blocking closure, checkout a backend for execute requests and pass
  it explicitly to the runtime/builders path.

Do not load the project per request. The existing global read-only project is
acceptable to keep for now; this plan is about execution concurrency, not model
tenancy.

**Verify**: `cargo build --bin xmla_proxy` -> exit 0.

### Step 5: Remove live-server dependency on the global backend helpers

After the refactor, inspect the live server path and delete or isolate the old
global-backend usage so future callers do not regress into it.

At minimum:

- `src/main.rs`, `src/execute/runtime.rs`, and the production path in
  `src/execute/builders.rs` must no longer depend on `Backend::get()`.
- If a helper remains global for old tests or benchmarks, leave a short comment
  explaining that it is legacy-only and not used by the server path.

This step is about making the architecture obvious in the code, not just making
tests pass accidentally.

**Verify**: `cargo test --lib` -> all pass.

## Test plan

- Add backend tests in `src/backend/mod.rs` for:
  - parallel reads through two independent file-backed backends
  - shared demo-data semantics across two independent demo backends
- Add execute-path tests in `src/execute/dispatch.rs` for concurrent cellset
  generation using injected backends; name these tests with a `concurrent_`
  prefix so `cargo test --lib concurrent_` remains a stable targeted command.
- Use the existing `FileQueryBackend` tests in `src/execute/dispatch.rs` as the
  structural pattern for backend injection.

## Done criteria

- [ ] `cargo build --bin xmla_proxy` exits 0
- [ ] `cargo test --lib concurrent_` exits 0 with the new concurrency tests
- [ ] `cargo test --lib` exits 0
- [ ] The live server path no longer depends on a module-level global backend
- [ ] Demo mode does not create one isolated in-memory database per request
- [ ] `src/main.rs`, `src/execute/runtime.rs`, and production execute helpers do
      not call `Backend::get()`
- [ ] Existing direct-SQL execution behavior remains intact for the current
      sample and converted-project tests
- [ ] `plans/README.md` status row updated

## STOP conditions

- The current in-scope code no longer matches the excerpts above.
- DuckDB file-backed connections cannot safely support the required parallel
  read behavior in this crate version.
- Making the demo path share one seeded dataset requires introducing a large new
  dependency or redesigning the benchmark/test helpers beyond this plan's scope.
- The refactor appears to require changing converter output, model semantics, or
  XMLA rowset contracts.
- The executor concludes that true multi-user correctness requires full XMLA
  session-state management, not just concurrent stateless execution.

## Maintenance notes

- This plan intentionally does **not** solve Malloy worker parallelism; the
  single Node worker in `src/engine/malloy_node_longlived.rs` can remain a
  compile bottleneck while the default direct-SQL path becomes concurrent.
- This plan intentionally does **not** remove the global project singleton.
  If the proxy later needs multiple loaded projects/models in one process, that
  should be a separate plan.
- Reviewers should scrutinize demo-mode behavior carefully. The easiest wrong
  refactor is replacing one shared seeded demo database with one fresh database
  per request, which would look concurrent but break semantic consistency.
- A future follow-up should replace the hardcoded shared XMLA `SessionId` in
  `src/xmla/response.rs`; that is a real multi-user correctness gap, but it is
  separate from the execution serialization this plan fixes.
