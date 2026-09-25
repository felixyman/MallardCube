# Plan 057 — The serving contract, qualified

## Status

- **Priority**: P1 (this is the plan that decides whether MallardCube is a staple)
- **Effort**: L
- **Risk**: MEDIUM (a schema is a public promise; a generator multiplies its mistakes)
- **Depends on**: 051 (budgets, settings), 055 (input contract), 056 (claims/probes)
- **Related**: 044 (boundary contract), 052 (aggregates), 053 (storage contract)
- **Category**: product / correctness

## Why

Two failure shapes make a BI engine untrustworthy, and both exist today:

1. **The config is a second hand-authored model.** `proxy-config.json` duplicates
   grain, keys, relationships and measure semantics that already live upstream
   (sqlmesh/dbt). It drifts, and every drift is a silent wrong answer.
2. **A wrong number can look plausible.** A database error on a query path must
   never render as zero or an empty cellset; a fan-out relationship must never
   double-count; a ratio measure must never be emitted as if it were additive.

The external review (2026-09-25) makes the serving contract its first item.
This plan keeps that ordering but **inverts the usual build order**: the
*validator* comes before the *generator*. A qualifier makes today's config
provably safe and becomes the generator's oracle; a generator built first just
produces more wrong answers, faster.

## A. The contract

One source-neutral file (`contract.yaml`, JSON Schema checked in CI) with only
the fields that are stable across sources and engines:

- `contract_version` (semver) and `model` (name, description);
- `grain` (fact table, key) and `dimensions` (id, key column, table, hierarchy
  levels or flat, date role, cardinality hint);
- `relationships` (fact, dimension, columns, cardinality: `many_to_one` only,
  active flag);
- `measures` (expression or upstream reference, declared aggregation:
  `sum` / `min` / `max` / `count` / `distinct_count` / `ratio` / `time_window`,
  format string, description);
- `display` (captions, ordinals, visibility) and `security` (role references);
- `provenance` (source system, source hash, generator name/version, timestamp).

Rules: no SQL dialect specifics in the contract — anything engine-specific is a
declared **serving hint**, and an unused hint is a qualification failure, not a
guess. The schema is versioned with a deprecation policy (shared with 061).

## B. The qualifier

`mallard qualify <config>` today emits a config/artifact readiness verdict
(`READY` / `PARTIAL` / `BLOCKED`). Extend it with **data-side checks**, all
executable, all exit non-zero on failure, each naming the check:

1. **Existence** — every referenced table/column resolves against the live
   database (`DESCRIBE`, `SELECT … LIMIT 0`).
2. **Key uniqueness** — `COUNT(*) = COUNT(DISTINCT key)` per dimension; report
   duplicate examples, not just a count.
3. **Fan-out** — for every relationship, the joined fact row count equals the
   fact row count; the join must not multiply rows. Reject `many_to_many` and
   wrong-direction cardinality with the measured multiplier.
4. **Orphans** — fact keys absent from their dimension (unless declared
   optional), because they silently drop from every pivot.
5. **Measure grain** — additive measures match a direct SQL aggregate; ratios
   recompute from their components; cumulative/time-window measures are
   verified per period; percentile/median style measures are refused unless a
   verification exists rather than assumed.
6. **Value oracles** — a bounded set of representative totals and grouped
   queries compared against direct SQL (whole-model, not per-measure), with an
   `--oracle <n>` mode for deeper runs.
7. **Fingerprint** — hash of schema + row counts + source mtimes, recorded in
   the verdict, printed at startup, exposed in `/status`, and part of the
   result-cache key so a data change cannot serve a stale number.

Output: a machine-readable verdict (JSON, stable shape) plus a human summary;
CI runs it for every fixture project and the demo.

## C. Fail closed

- Audit every query path for swallowed errors (`unwrap_or(0)`, `ok()`, ignored
  `Result`) and give the engine calls typed errors; a failure answers a
  sanitized SOAP fault, never a number.
- **Fault-injection matrix** (tests): missing table, missing column, locked or
  corrupt database file, interrupted query, missing/stale aggregation sidecar,
  reload mid-flight. Every case must fault — with the class of failure in the
  message, and without leaking paths or credentials.
- Finish the **consumption audit** from plan 051: every requested set, filter,
  measure, property and option is consumed or faults; the plan and renderer
  mark each entry, and a leftover faults naming it.
- Faults carry a stable code and a hint; `/status` exposes the last error.

## D. Generators (only after B is green)

1. `sqlmesh → contract`, against a real SQLMesh fixture (the upstream default);
2. `dbt → contract`;
3. explicit YAML for small teams;
4. Cube only if its metadata maps cleanly; Superset as a consumer of the same
   governed marts, not a native adapter.

Generation is gated: generate → `qualify` → refuse to write on failure.
Provenance is embedded; CI asserts the checked-in contract equals the generated
one, so no hand edits survive.

## Scope

**In**: contract schema + JSON Schema, qualifier extensions, fingerprint
plumbing, fault closure, fault-injection matrix, consumption audit, generators
and their fixtures, CI wiring.

**Out**: column-level security (058), deployment/observability (059), MDX
features (060), live attach and object-store intake (061), aggregate design
(052), storage contract (053).

## Done criteria

- A seeded defect (missing column, duplicate key, fan-out, orphan, wrong grain,
  wrong ratio, dead database) fails `qualify` or faults, and CI proves each.
- The fault-injection matrix passes: no data-side failure answers a number.
- A real SQLMesh fixture and a dbt fixture each generate a contract that
  qualifies unedited and passes the Excel sweeps plus `probe-fidelity.sh` and
  `probe-parity.sh`.
- `/status`, logs and cache keys carry the contract version and database
  fingerprint.

## STOP conditions

- If a contract field cannot be expressed without engine-specific semantics,
  keep it as a declared serving hint and **refuse** the query when the hint is
  required but unused — never guess.
- If a generator is not byte-deterministic across runs, fix determinism before
  adding the next generator.
