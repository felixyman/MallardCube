# Plan 054 — Engine readiness: dialect seam, capabilities, and the protocol-overhead gate

## Status

- **Priority**: P2 (enabler for a post-DuckDB performance path; no user-visible change by itself)
- **Effort**: M
- **Risk**: LOW (behaviour-identical; no new engine ships here)
- **Depends on**: 050 (measurements), 051 (streaming/formatting)
- **Related**: 044 (boundary contract), 052 (aggregate artifacts), 053 (intake)
- **Category**: architecture / performance

## Why this matters

Single-node DuckDB is the default, not the intended ceiling. On-prem teams that
outgrow one node will reach for ClickHouse-class engines, and the swap should be
a config change plus one SQL emitter — not a rewrite. The measured gesture set
says exactly where that pays off:

| gesture | today (50M rows) | dominant cost | a faster engine buys |
|---|---|---|---|
| single/two-field pivot, rollups on | 4 ms | proxy ~1–3 ms, engine ~1 ms | little (already near the floor) |
| Calendar / Full Date drilldown | 215–370 ms | engine scan | large (10–100×) |
| drillthrough | 98 ms | engine scan + LIMIT | large |
| Top-N / value filters | engine-bound | engine | large |
| 200k-member cache build | 1.1–2.8 s, 156 MB | **proxy + network** (~5 µs and ~140 MB/s per row) | ~nothing |
| Discover / metadata | 0–13 ms | proxy + cache | nothing |

So an engine swap is cheap precisely where the engine dominates, and worthless
where the response is proxy-bound. Two consequences drive this plan: keep the
SQL-emission and execution boundary clean so the swap stays cheap, and make the
proxy's own cost a measured, gated number so "can the XML generation keep up?"
is answered with data rather than hope. With a *remote* engine, metadata queries
also stop being local calls, so the dictionary/result caches (plans 031, 032,
053) become load-bearing rather than optimisations.

### Decisions recorded here (so they are not re-litigated per feature)

- The **core stays MIT** and depends only on OSS engines with permissive
  licences (DuckDB MIT, ClickHouse Apache 2.0). No proprietary backend in the
  critical path.
- **One engine per proxy instance** (`dialect` in the config, 1:1 with one
  database). **No federation** across engines inside one model — a user who
  needs two engines runs two proxies. Record this in plan 044's boundary
  contract.
- **On-premises, no-internet deployments are the target**; nothing in the query
  path may require egress or a hosted service.
- ClickHouse is the named candidate for engine #2; Trino/StarRocks/Postgres are
  later candidates. **This plan builds none of them.**

## Design

### A. Seam-lite now, extraction later

`Dialect` exists (`engine/model.rs:16`) with one variant and is never read; the
real SQL boundary is `engine/sql.rs`, which is already almost the only place
that knows SQL. Keep it that way instead of extracting a trait before a second
implementation exists ("rule of three"):

- All SQL strings (predicates, casts, date windows, rollup DDL) live in the
  emitter module; no dialect conditionals elsewhere.
- No engine types (`duckdb::`) outside the engine/backend module.
- Enforce both with an **allowlist test** over the query path (source scan with
  a small, explicit allowlist), not a brittle whole-crate grep.

When engine #2 lands: lift the emitter into a per-dialect object, add
capabilities (below), and run the corpus (F). The frontend, renderers, Discover
handlers and the Excel surface do not change.

### B. Capabilities as data

The frontend already gates fallback SQL with `FallbackCapability`
(`engine/model.rs:23`). Generalise the idea into a per-engine capability set the
planner consults before choosing a lowering:

| capability | why it matters |
|---|---|
| window functions | time-intelligence lowerings (YTD/QTD/rank) |
| `FILTER (WHERE …)` on aggregates | conditional measures without `CASE` rewriting |
| date truncation / `INTERVAL` forms | Calendar level grouping |
| strict typing | DuckDB silently casts `'1'`→1; ClickHouse does not |
| `DISTINCT` scaling | dimension dictionaries (`plan 031`) |

Missing capability = pick another lowering, fall back to the fact path, or
return a clear XMLA fault. Never silently wrong numbers.

### C. Engine settings surface

One small struct describing what the engine may consume, mapped per engine,
instead of baking DuckDB's knobs into the product:

| setting | DuckDB (today) | ClickHouse (later, documented not built) |
|---|---|---|
| memory ceiling | `SET memory_limit` | `max_memory_usage` |
| spill area | `SET temp_directory` | n/a (server-side) |
| threads | `SET threads` | `max_threads` |
| query timeout | watchdog + `Connection::interrupt()` | `max_execution_time` |

- The **default memory ceiling is cgroup-aware**: read
  `/sys/fs/cgroup/memory.max` (v2) or `memory/memory.limit_in_bytes` (v1) and
  use 70% of it. On Kubernetes the pod limit is the truth; "a share of host
  RAM" is wrong there. When no cgroup limit is set, **leave the engine's own
  default alone** rather than recompute one from host RAM — DuckDB already
  derives its default that way, and setting a lower number would be a silent
  regression outside containers.
- `MALLARDCUBE_*` environment variables are overrides on top of the computed
  defaults.
- `/status` reports the effective values and where each came from
  (`cgroup | env | default`), so "why did it OOM at 8 GiB?" is answerable from
  the pod itself.

*Implemented 2026-09-23 in `src/engine/settings.rs` (first increment of 051-A):
resolution ladder, cgroup parsing, `/status` block, and an integration test that
the values reach a real DuckDB connection. Until the shared-engine change lands,
the ceiling applies per pooled connection rather than process-wide.*

### D. Protocol-overhead gate (the "can XML generation keep up?" number)

Add a **stub engine** (bench/test only, never a supported dialect) that returns
precomputed results instantly, then measure the proxy alone:

- `n` members and `n` cells per request, reported as ms/1k members and
  ms/1k cells, plus peak RSS.
- The current estimate is ~5 µs and ~140 MB/s per member row (156 MB in ~1.1 s
  warm); the first stub run replaces the estimate with a baseline.
- Run report-only once, then pin the gate (target: the pinned number plus a
  generous margin; plan 051's streaming is expected to move it substantially).
- This is the yardstick for "would a faster engine actually help this gesture?"
  and the guard against a slow engine waiting on a slower proxy.

### E. Serialization budget

Owner for the response-side wins plan 051 starts:

- pre-sized buffers and direct number formatting (`itoa`/`ryu` style) instead of
  `format!` per value;
- no intermediate `Vec<String>` per row where a single buffer will do;
- streaming writes (051-C) so the budget is memory-constant;
- optional response compression on the XMLA body — **verify first** that
  MSOLAP/Excel accepts it (the mirror is reachable for exactly this check)
  before enabling by default.

### F. SQL conformance corpus

Commit golden SQL for the gesture set (the nine captured Execute statements,
plus one statement per Discover/MDX shape the frontend emits), as normal tests:
today they pin DuckDB SQL shape against accidental refactors; later they become
the per-dialect conformance suite (`sql.duckdb.golden`, `sql.clickhouse.golden`,
…). Adding engine #2 is then: emitter + capability set + one golden column +
the value oracles run against that engine.

### G. Boundaries

Record in plan 044: one engine per instance, no cross-engine joins, no
distributed planner, no engine-specific SQL outside the emitter. The proxy stays
the MDX/XMLA brain and Excel-fidelity layer; the engine is a deployment choice.

## Scope

**In:** allowlist test, capabilities data model, engine settings struct with the
cgroup-aware default, stub engine + protocol-overhead measurement (report-only,
then pinned), serialization budget items owned with 051, SQL golden corpus,
boundary notes in 044.

**Out:** implementing ClickHouse or any other engine; emitter trait extraction
before a second engine exists; federation; a distributed query planner;
deployment/packaging specifics (separate, deferred).

## Done criteria

- The allowlist test fails if engine-specific SQL or `duckdb::` types appear
  outside the allowed modules, and passes on the current tree.
- `/status` reports the effective engine settings and their source; a container
  with a 2 GiB limit reports a limit derived from the cgroup, not the host.
- The bench prints ms/1k cells and ms/1k members from the stub engine; the first
  baseline is recorded (plan 050 evidence) and a gate is pinned afterwards.
- Golden SQL tests exist for the gesture corpus and fail on unexplained SQL
  changes.
- No behaviour change: 516+ tests, oracle corpus, proxy smoke and the 50M bench
  within noise; the Excel surface is untouched.

## STOP conditions

- If the cgroup-aware default needs changes to the pool topology, land plan
  051-A first and keep this plan to discipline, capabilities and the gate.
- If the allowlist test starts fighting legitimate uses (diagnostics, doctor
  commands, fixtures), narrow it to the query path rather than maintaining a
  growing exception list.
- If the first stub-engine baseline shows the proxy already dominates every
  gesture, stop and make the response path (051-C/E) the priority — a second
  engine would buy nothing yet.
