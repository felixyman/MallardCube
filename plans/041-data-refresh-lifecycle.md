# Plan 041: Data refresh lifecycle — writer exclusivity, freshness signal, reload

## Status

- **Priority**: P1 (production blocker for file-backed deployments)
- **Effort**: L (three phases; Phase A is S, B is M, C is L)
- **Risk**: MEDIUM (Phase C swaps live runtime state)
- **Depends on**: 042 (no production path may read the demo backend before the
  pool is made swappable)
- **Category**: operations / correctness-adjacent

## Why this matters

MallardCube's locked adoption path is "point it at your existing DW, retire
SSAS with zero data movement". A DW is loaded on a schedule. But a running
MallardCube **holds the DuckDB file and never lets go**:

- `BackendSource::file` pre-opens a pool of read-only connections at startup
  (`src/backend/mod.rs`, `BackendPool::open`) and nothing ever reopens them —
  there is no reconnect, file-watch, or reload path anywhere in the tree.
- DuckDB's file lock is exclusive between a writer and readers. Verified
  2026-09-20 on this machine:

  ```
  $ duckdb <proxy db> "CREATE TABLE lock_probe(i INT)"
  IO Error: Could not set lock on file "...": Conflicting lock is held in
  .../mallard (PID 375869) by user felix.
  ```

  and the reverse — a read-only open while a writer holds the file fails the
  same way, so the proxy cannot even start while a load job is running.

- Consequences:
  1. A load job fails while the proxy runs (or the proxy fails to start while
     the load runs) — no documented ordering, no friendly error.
  2. The proxy never sees new data until the process restarts, even after a
     successful load.
  3. If the load job replaces the file by rename, the proxy's open handles keep
     the old inode and serve yesterday's data indefinitely — silently.
  4. The aggregation sidecar (`src/engine/aggregate.rs`) has a size+mtime stamp,
     but it is only checked at startup; `parent_child.refresh` is startup-only;
     the 5 s result cache (plan 032) is the only *documented* staleness.

- Nothing in `README.md`, `docs/`, or the site explains any of this: there is
  no refresh runbook, no freshness signal, and no reload path.

This is the first thing a departmental team hits in production, in one of three
ways: the load job errors with a lock message, the proxy won't start after the
load, or nobody notices the data is a day old.

## Current state (verified 2026-09-20)

| Piece | Where | Behaviour |
|---|---|---|
| Pool open | `src/backend/mod.rs` `BackendSource::file` → `BackendPool::open` | N read-only connections, opened once, round-robin checkout |
| Aggregations | `src/engine/aggregate.rs:41` `static AGGREGATIONS: OnceLock<Vec<Aggregation>>` | Built once in `main.rs:291`; pool attaches the sidecar when non-empty |
| Agg stamp | `src/engine/aggregate.rs:133` (`agg_meta`: size + mtime + version) | Checked only by `ensure_aggregations` at startup |
| Parent-child | `src/project/project.rs:388` (`ParentChildMode::Materialize`) | Runs at load; `refresh: true` forces a rebuild at next start |
| Result cache | `src/execute/cache.rs` | 5 s TTL, no `clear()` yet |
| Startup failure | `src/main.rs:307` | `panic!("failed to configure DuckDB: {path}")` — no lock hint |
| HTTP surface | `src/main.rs:339-342` | only `POST /xmla` |

## Design

### Phase A — Document the constraint and fail loudly (S)

- `README.md` + `site/src/content/docs/deployment.mdx`: a **"Refreshing data"**
  section with the verified lock semantics and a runbook:
  1. load into a staging file (`build.duckdb`),
  2. `mv build.duckdb live.duckdb` (atomic rename; readers keep the old inode),
  3. restart the service (`systemctl restart mallard` / `docker compose restart`)
     or, once Phase C lands, `systemctl reload mallard` (SIGHUP).
- Replace the startup panic with an actionable message: detect the lock error
  and say "another process (a load job?) holds `<path>` — DuckDB allows one
  writer or many readers, not both; stop the writer and retry".
- Document that `MALLARDCUBE_AGG_CACHE` rollups are rebuilt on restart when the
  source stamp changed, and that a data refresh never touches the model file.

### Phase B — Freshness signal (M)

- Keep a small `DataStamp { path, size, mtime, loaded_at }` captured when the
  pool opens (reuse the same stat logic as `aggregate.rs`).
- New endpoints on the existing listener:
  - `GET /health` → `200 ok` (liveness; no data).
  - `GET /status` → JSON: `catalog`, `cube`, `db_path`, `db_size`,
    `db_mtime`, `loaded_at`, `pool_size`, `result_cache` counters,
    `queries_total`, `started_at`.
  - Auth: when `auth.trusted_proxy` is configured, require the trusted header
    (deny closed) exactly like `/xmla`; otherwise local-only deployments get it
    unauthenticated (documented).
- Startup log: print the stamp (`🗄️ DuckDB: <path> (loaded <mtime>, <size>)`).
- Optional stretch: expose the stamp as a `DISCOVER_PROPERTIES` entry
  (`DataLastLoaded`) so Excel-side operators can see it without `/status`.

### Phase C — Reload without restart (L)

Goal: `kill -HUP <pid>` (or `systemctl reload mallard`) re-reads the data file
and rebuilds what depends on it, without dropping in-flight requests.

- **Swap the pool**: `AppState.backend_source` is a `BackendSource` cloned per
  request. Wrap it in `ArcSwap<BackendSource>` (or `RwLock<Arc<...>>`) so a
  request snapshots the current pool; in-flight requests keep the old pool
  alive until they finish, new requests get the new one.
- **Aggregations**: turn `static AGGREGATIONS` into swappable state
  (`ArcSwap<Vec<Aggregation>>`). On reload, re-run `ensure_aggregations`; if the
  stamp is unchanged this is a no-op (cheap), otherwise the sidecar is rebuilt
  to a temp file and renamed, and the new pool attaches it.
- **Result cache**: add `ResultCache::clear()` and call it on reload (stale
  entries must not survive a data swap).
- **Parent-child**: re-run `prepare_parent_child` in `Refresh` mode when the
  dimension is materialized. Hierarchy *shape* changes (new depth) still need a
  model rebuild → document "restart for model changes; reload for data changes".
- **AutoModel (`MALLARDCUBE_DB`)**: detection and `date_dim` seeding happen at
  startup; keep reload as `PROXY_CONFIG`-only and document that AutoModel
  deployments restart (or re-run detection in a later increment).
- **Trigger**: install a `SIGHUP` handler (`tokio::signal::unix`); on Windows
  there is no SIGHUP — document `MALLARDCUBE_RELOAD_WATCH=<secs>` as the
  cross-platform alternative (poll the stamp and reload when it changes; also
  covers the "ETL swapped the file" case automatically).
- **Observability**: log a `RELOAD` line with old/new stamps and what was
  rebuilt; `/status.loaded_at` moves.

## Scope

**In scope**: the docs runbook, the lock-aware startup error, `/health` +
`/status`, `DataStamp`, SIGHUP reload of pool + aggregations + result cache +
parent-child refresh, the optional watch mode, and tests.

**Out of scope**: multi-project reload (ARCH-01), model/config hot-reload,
AutoModel re-detection, write-back, and attached sources (Phase 4 — those make
the whole problem disappear and should be prioritized separately for
production).

## Risks

- **Swap races**: a request must never see a half-swapped state. ArcSwap gives a
  consistent snapshot; never hold the swap lock across a query.
- **Two pools briefly alive**: bounded by pool size; the old pool drops when its
  last request finishes. Document the transient memory.
- **Sidecar rebuild**: must write to a temp file and rename so old connections
  never read a mutating file.
- **SIGHUP default action is process termination**: the handler must be
  installed before the service is exposed; test it in a smoke run.
- **Freshness vs result cache**: after a reload the cache is cleared, so no
  request can serve pre-reload rows.

## Test plan

- Unit: `DataStamp` capture/compare; `ResultCache::clear()`; reload swaps the
  pool against a temp DuckDB whose file is replaced between requests (query
  before → old value, reload → new value, no restart).
- Integration: file-backed fixture (`data/generated.db`, 10 rows) — replace the
  file with one holding different rows, call the reload function, assert the
  next query returns the new value and `/status.loaded_at` advanced.
- Smoke: extend `scripts/proxy-smoke.sh` with a reload step (SIGHUP, then assert
  a value changed after swapping the DB) — guarded so it only runs when the
  script starts the server itself.
- Docs test: the runbook must be executable as written (staging + rename +
  restart) — do it once by hand and record the result in the plan index.

## Done criteria

- [ ] README + site deployment page document the lock, the runbook, and the
      aggregation/result-cache staleness windows.
- [ ] Startup failure names the lock holder problem instead of panicking blankly.
- [ ] `/health` and `/status` answer; `/status` carries `db_mtime` and
      `loaded_at`; both are auth-gated when auth is configured.
- [ ] `SIGHUP` reloads data with no restart: new file content is served, the
      result cache is cleared, aggregations rebuild only when the stamp changed.
- [ ] In-flight requests during a reload complete against their original pool.
- [ ] All existing tests pass; new reload tests cover the swap.
- [ ] Plan index updated (and 042 landed first).
