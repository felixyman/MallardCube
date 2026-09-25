# Plan 057 — The serving contract, qualified

## Status

- **Priority**: P1 (this is the plan that decides whether MallardCube is a staple)
- **Effort**: L
- **Risk**: MEDIUM (a schema is a public promise; a generator multiplies its mistakes)
- **Depends on**: 051 (budgets, settings), 055 (input contract), 056 (claims/probes)
- **Related**: 044 (boundary contract), 052 (aggregates), 053 (storage contract)
- **Category**: product / correctness

**Build order: C → B → A → D.** Fail closed, then qualify today's config,
then design the contract against the qualifier's real needs, then generate.
The first two sections have zero contract surface; the schema is deliberately
last so it is derived rather than invented.

## Why

Two failure shapes make a BI engine untrustworthy, and both exist today:

1. **A wrong number can look plausible.** A database error on a query path must
   never render as zero or an empty cellset; a fan-out relationship must never
   double-count; a ratio measure must never be emitted as if it were additive.
2. **The config is a second hand-authored model.** `proxy-config.json`
   duplicates grain, keys, relationships and measure semantics that already
   live upstream (sqlmesh/dbt). It drifts, and every drift is a silent wrong
   answer.

The external review (2026-09-25) makes the serving contract its first item.
This plan keeps that goal but **inverts the usual build order**: correctness
and the validator come first. A qualifier makes today's config provably safe
and becomes the generator's oracle; a generator built first just produces more
wrong answers, faster.

## C. Fail closed (do first)

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

## B. The qualifier (do second)

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

The checks above are what the contract must be able to express — they are the
schema's requirements, which is why they come before it.

## A. The contract (do third)

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

### Compatibility policy — changeable while pre-alpha

The contract is a **projection target, not a public interface** until 1.0. The
rules that keep it changeable:

- **`0.x` means unstable**: minor bumps may break, a changelog announces it,
  and there is no deprecation window until 1.0. 1.0 is the commitment point and
  is tied to Gate G1 and plan 060's certification, not to this plan.
- **Annotations escape hatch**: an `annotations:` namespace the proxy ignores
  and generators preserve, so prototyping a new field does not churn the
  schema. A field earns core status only with a qualifier check behind it.
- **Fail closed on meaning**: an unknown *core* field is a hard error (a rule
  you think is enforced but isn't is worse than no rule); a contract version
  newer than the build supports is refused with an upgrade hint; older
  versions go through an explicit migration command, never silent tolerance.
- **The runtime never reads the contract** — it serves the generated
  projection, so schema churn touches generators and validation only.
- **Version fixtures**: one contract fixture per supported version, each
  required to keep qualifying; dropping a version is a deliberate deletion.
- **Nothing external pins it**: no schema URL, no "certified against this
  contract version" anywhere, and the docs mark the schema unstable.
- No SQL dialect specifics in the contract — anything engine-specific is a
  declared **serving hint**, and an unused hint is a qualification failure, not
  a guess.

## D. Generators (do last)

1. `sqlmesh → contract`, against a real SQLMesh fixture (the upstream default);
2. `dbt → contract`;
3. explicit YAML for small teams;
4. Cube only if its metadata maps cleanly; Superset as a consumer of the same
   governed marts, not a native adapter.

Generation is gated: generate → `qualify` → refuse to write on failure.
Provenance is embedded; CI asserts the checked-in contract equals the generated
one, so no hand edits survive.

## Scope

**In**: fault closure (typed errors, fault-injection matrix, consumption audit),
qualifier extensions, fingerprint plumbing, the contract schema plus its
compatibility policy and version fixtures, generators and their fixtures, CI
wiring.

**Out**: column-level security (058), deployment/observability (059), MDX
features (060), live attach and object-store intake (061), aggregate design
(052), storage contract (053), publishing the schema as a stable interface.

## Done criteria

- The fault-injection matrix passes: no data-side failure answers a number.
- A seeded defect (missing column, duplicate key, fan-out, orphan, wrong grain,
  wrong ratio, dead database) fails `qualify` or faults, and CI proves each.
- The contract exists at `0.x` with the annotations namespace, the version
  rules and at least one fixture; an unknown core field and a newer version
  both refuse with a clear message.
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
- If a schema change would require an external consumer to migrate, it is too
  early for that change: keep it in annotations until 1.0.

## Progress

- **2026-09-25 — section C, first slice: a failed query can no longer look
  like a number.** `Backend` records the first failure per connection in every
  query method (`query_scalar`, `query_count`, `query_grouped_1d`, `pairs`,
  `grouped_n`, `strings`, `rows`, `column_names`), the `QueryBackend` trait
  exposes `take_failure`, and the request paths fault: the cellset runtime
  checks twice — before caching (a failure is never cached) and after rendering
  (a member-dictionary failure replaces the rendered cellset) — and the member
  builder checks after its dictionary queries. Each use site clears the latch
  first so a failure from an earlier request on a pooled connection cannot
  fault an unrelated one. NULL stays a value, not a failure.

  Verified live with a scratch model whose measure reads a missing column:
  `value=0` became `a query against the database failed: Binder Error:
  Referenced column "no_such_column" not found`, on both the total and the
  group-by shape, with the healthy demo unchanged.

  Two traps worth remembering: the trait must *forward* the latch (a default
  `None` silently disarms it — exactly the bug this slice fixed), and shared
  test fixtures latch across parallel tests, which is why the clear is
  per-use rather than per-connection.

  Still open in section C: the `Result`-typed engine API (the latch is the
  interim, not the destination), the rest of the fault-injection matrix
  (locked/corrupt file, interrupted query, sidecar, reload mid-flight), and the
  consumption audit.

- **2026-09-25 — section C, matrix slice**: a missing database file and a
  corrupt one both fail to open (startup refuses rather than serving zeros),
  and an interrupted query — the request-timeout path — records a connection
  failure that the request path faults on. Still open in the matrix: a
  missing/stale aggregation sidecar and a reload mid-flight. Then the
  `Result`-typed engine API (the latch is the interim) and the consumption
  audit close section C.

- **2026-09-25 — section B, first slice: `mallard qualify` runs data-side
  checks.** Over the physical shape only (fact tables, the dimension tables
  relationships name, and dimensions that declare their own `table_name`):
  every table must be readable; every relationship's dimension key must be
  unique; no relationship may fan out (joined rows ≤ fact rows, reported with
  the measured multiplier); orphan fact keys are a PARTIAL finding. Dimension
  keys come from relationships (`dim_column`), never from `physical_field` —
  that is a display path, and checking it produced false "not unique" findings
  on the shipped fixtures.

  Verified: `generated_retail_analytics` is READY; a seeded-defect database
  (duplicate key, fan-out, orphan, missing table) trips all four findings in a
  test; `generated_contoso` is now BLOCKED because its dummy database lacks
  `promotion` and other tables its model references — a real fixture gap for
  plan 045 that a PARTIAL verdict used to hide.

  Still open in section B: the database fingerprint, measure-grain checks
  (ratios, cumulative windows), value oracles (`--oracle n`), and the
  machine-readable JSON verdict.

- **2026-09-25 — section B, fingerprint slice.** A process-wide data epoch is
  bumped when the source opens and on every reload; it is part of the result
  cache key and reported in `/status` (`data.epoch`), so a log line and a cache
  key correlate and a reload can never serve pre-reload rows even if a cache
  clear were missed. Verified live: `/status` carries the epoch, and a test
  proves the key changes across a bump. Still open in section B: measure-grain
  checks (ratios, cumulative windows), value oracles (`--oracle n`), and the
  machine-readable JSON verdict.

- **2026-09-25 — value oracles.** `oracles.json` beside the config carries
  hand-written expectations (measure, optional dimension/member slice, expected
  value, tolerance); `qualify` runs each through the proxy's own SQL emitter and
  compares. A wrong expectation and an unknown measure both block, and a test
  proves both. The demo file ships values recorded from the reference engine
  (total 521,586,767; Automotive 25,102,648) — independent of the proxy, which
  is the whole point of an oracle.

  Running the checks also found a stale fixture relationship in project3
  (`order_date -> date_dim.full_date`, a column `sales_fact` does not have);
  removed, with the gates as the safety net.

  Still open in section B: measure-grain checks (ratios, cumulative windows)
  and the machine-readable JSON verdict.
