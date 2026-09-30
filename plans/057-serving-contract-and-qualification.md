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
- `grain` (fact table, key, serving id, measure group) and `dimensions` (id,
  table, key, attribute — the member value column Excel names members by; for
  a date role, the leaf — hierarchy name, caption, ordinal, visibility, date
  role, cardinality hint, levels with their cardinality hints);
- `relationships` (fact, dimension, columns, cardinality: `many_to_one` only,
  active flag);
- `measures` (expression or upstream reference, declared aggregation:
  `sum` / `min` / `max` / `count` / `distinct_count` / `ratio` / `time_window`,
  format string, description, `valid_grain` for identity aggregates);
- `time_intelligence` (date dimension plus the flag columns for the windows the
  engine serves) and `security` (role references);
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

- **2026-09-28 — section A, first slice: the contract exists at 0.1.**
  `schema/contract-0.1.json` (JSON Schema draft 2020-12, `additionalProperties:
  false` on core objects, `annotations` free), a hand-written fixture
  (`contracts/upstream_marts/contract.yaml`) mirroring the thin projection's
  grain, keys, serving names, relationships, measure declarations and flag
  catalogue with no SQL, and `mallard contract validate <file> [--json]`
  (serde `deny_unknown_fields`, duplicate-mapping-key refusal, the version
  rules, and cross-field shape checks). The CI test job validates every
  fixture against the schema (`scripts/contract_check.py`) and runs the Rust
  validator; the docs page is marked unstable; twelve tests pin the fixture
  and each refusal.
  Review round (reviewer subagent) found and this slice fixed: the fixture
  dropped the median mart's Category join and both gates blessed it; a
  `valid_grain` measure could be declared additive (`sum` on the median);
  duplicate YAML keys were last-wins in both gates (now refused by walking the
  document before the typed parse); the Rust validator was weaker than the
  schema (empty collections/keys, short join pairs, empty names); version
  parsing accepted `0.1`/`00.1.0`/`0.1.0-beta` and mislabelled an unparsable
  minor as older; the dimension attribute column (`physical_field`), the
  serving names and the model-level flag catalogue were inexpressible (now
  `attribute`, `hierarchy_name`, grain `id`/`measure_group`, and typed
  `time_intelligence` flags); the Date key contradicted the key attribute (now
  `attribute: full_date`, `key: date_key`); cross-references (time_window
  dimension, valid_grain columns, join column vs key, one-table-one-key) were
  unchecked. Recorded leftovers: `expression` text is not parsed — the
  qualifier's `--contract` mode is where it becomes checkable —
  `security.roles` binds only at projection/deployment time, and an
  `active: false` relationship is declared but not yet consumed (the
  projection decides how an inactive join is served).
  Second review round (reviewer subagent on the fix commit) found the
  "validator ≥ schema" claim false for five fields the schema refuses and the
  validator accepted (empty `expression`/`reference`/`valid_grain`, empty
  grain `id`/`measure_group`), and that a window measure could bind to any
  flag with no catalogue (or to another date role's catalogue). Fixed:
  emptiness-aware checks, `valid_grain` distinguishes declared-empty from
  absent, a `time_window` now requires the flag catalogue and must name the
  catalogue's date role, YAML merge keys (`<<`) are refused with a clear
  message (the schema checker expands them and the validator would not, so
  the gates would read different documents), and the schema now carries the
  additivity rules as `if`/`then`. `scripts/contract_check.py` grew a
  19-case conformance corpus that runs both gates over the same cases and
  fails on any divergence, so "at least as strict" is enforced by
  construction. One known bounded divergence, recorded rather than papered
  over: custom YAML tags (e.g. `!foo`) are refused by the schema checker
  (standard YAML) but `yaml_serde` strips tags before the validator sees them,
  and its public API exposes no event stream to detect them; standard tags
  (`!!str`, `!!int`) behave identically in both gates.

- **2026-09-28 — section A, step 2: the projection.**
  `mallard contract project <file> [--catalog X] [--cube Y] [--db-path P]
  [--out PATH] [--json]` turns a validated contract into the proxy config the
  runtime reads (`src/tools/contract_project.rs`): grain becomes fact tables
  (serving id, measure group), `attribute`/`key` become the dimension's member
  and join columns, relationships become joins, declarations become SQL
  expressions (`sum` → `SUM(col)`, ratios verbatim, `max`/`min` over a
  per-grain mart for identity aggregates), and `time_intelligence` becomes the
  flag catalogue (calendar slots from levels named Year/Quarter/Month).
  Deployment specifics are parameters, never contract fields. Shapes the
  config cannot express are refused: an unexpanded upstream `reference`, an
  inactive relationship, a composite date key. The round-trip test projects
  the fixture and asserts it reproduces
  `projects/upstream_marts/proxy-config.yaml` semantically; the projected
  config passes `qualify --strict` with the same verdict and notes as the
  checked-in one. Next in section A: the qualifier's `--contract` mode, then
  the generators.
  Review round (reviewer subagent on the projection commit) found and this
  slice fixed: a dimension's declared table survived only through a
  relationship, so an unbound dimension was silently served from the primary
  fact table and a second date role from the global date table (both now
  refused with messages); the flag-column defaults were misread as inventing
  names for omitted slots (the field-level default actually wins over the
  container default, so a missing key inside a present block is unavailable —
  now pinned by a test, and the real non-idempotence was a format-less
  measure: `format_string: ''` became the default on reload, fixed by
  emitting the canonical `normalize` → `deminimize` form, with an idempotence
  test); the round-trip test compared five sections and missed
  roles/auth/dialect/section files (now the whole config, plus a
  display-field test); `--json` was absent on the serialize/write failure
  paths and one verdict label conflated invalid contracts with
  config-inexpressible shapes (now `invalid`/`refused`/`error` on every path,
  notes included); the projection refuses project-only flags under `validate`
  and an empty `--out`, guards relationship arity and unknown dimensions for
  library callers, and notes role references without bindings and a missing
  `--db-path`. Recorded leftover: DISCOVER MEASURE_AGGREGATOR is pinned to
  Sum for every measure — the count/min/max/distinct_count codes need the
  reference oracle (the VM was down), after which the projection maps them
  and the checked-in config follows (resolved 2026-09-29 by measurement; see
  the entry below). The provenance-header idea was dropped:
  `fmt` rewrites drop comments, so a header would break `fmt --check`
  canonicality; the config cannot carry provenance, grain keys or the model
  description, and the docs now say so.
  Third review round (verification of the fix commit) confirmed the binding,
  date-table, canonical-form and verdict fixes, and found three surviving
  gaps, all now closed: the output format followed YAML regardless of the
  target path (`--out x.json` wrote YAML that the loader then refused — the
  format now follows the extension, like `fmt`); a second date role on the
  same table but a different full-date column would filter the global column
  (now refused alongside the table check); and a dimension joined differently
  on two facts was accepted although the engine joins through the first
  relationship (disagreeing joins are now refused). The security note now says
  the file has no `auth` block and the roles enforce nothing until the
  deployment adds auth and table permissions; `--json` keeps its key set on
  the usage-error paths too and carries the notes on write failure; `--out`
  warns when it overwrites an existing file; and the new refusals are pinned
  by tests (join arity, unknown dimension, disagreeing joins, date-role
  column, verdict key set, a `.json` target). Recorded leftover: degenerate
  dimensions (attribute on the fact table, no relationship) are refused — the
  runtime's fallback is only sound when the query's fact is the primary one.

- **2026-09-28 — section A, step 3: the qualifier's `--contract` mode.**
  `mallard qualify <config> --contract <contract.yaml>` validates the contract
  and runs its declarations against the same database
  (`src/tools/contract_qualify.rs`), folded into the verdict with a
  `contract:` prefix and reported as `contract_file` in the JSON: every
  declared grain key must be unique in its table (NULL keys are a partial
  finding), every `valid_grain` must be unique in the source table (the
  identity-aggregate hazard), every measure's projected SQL must bind
  (`EXPLAIN`), every dimension attribute/level column and every
  time_intelligence flag must exist, and the served config must be the
  projection of the contract — a stale or hand-edited config is blocked with
  "regenerate the projection", while extra served entries are partial
  findings. Deployment-specific parts (`auth`, catalog/cube/db path, extra
  role bindings) are deliberately not compared. Tests build a synthetic
  DuckDB and also rebuild the upstream demo from its SQL files, so the
  fixture's contract is qualified end-to-end in CI without the duckdb CLI.
  This closes the recorded leftover that `valid_grain` was checked by nothing.
  Review round (reviewer subagent on the qualification commit) found and this
  slice fixed: the projection check was gated behind a usable database, so a
  stale config passed with exit 0 on a fresh checkout (correspondence now runs
  unconditionally, with a test); the whole-entry comparison blocked on
  presentation fields the contract cannot express (`display_name`, `units`,
  numeric precision/scale, dimension descriptions, All/leaf names, a fact's
  `source_name`) — those are excluded and documented as the deployment's; an
  identifier containing a quote produced a misleading SQL error (the
  `data_findings` guard is ported); an empty grain table qualified vacuously
  (now a partial finding); and the NULL-key partial's narrowness plus the
  valid_grain NULL reasoning are documented in code. Recorded: role
  *permissions* are the deployment's (the check verifies declared role names
  are served, not what they filter); `provenance.source_hash` is not verified;
  the relationship-cardinality declaration has no config counterpart.
  Next in section A: the generators (D).

- **2026-09-29 — the aggregator leftover, resolved by measurement (VM up).**
  Deployed five reference models covering `SUM`, `SUMX`, `COUNTROWS`,
  `COUNT`, `DISTINCTCOUNT`, `MIN`, `MAX`, `AVERAGE`, `DIVIDE`, `TOTALYTD` and
  a bare column reference at compatibility 1600 and 1700: every explicit
  tabular measure reports `MEASURE_AGGREGATOR` **0 (Unknown)**, whatever its
  DAX; only the hidden `__Default measure` reports 127 (Calculated) with
  `MEASURE_IS_VISIBLE=false`. The spec's 1/2/3/4/8 values are the
  multidimensional enumeration. Fixed: the config default follows the
  measurement (`default_aggregator() -> 0`), auto-model stops writing 1/2,
  the projection's "pinned to Sum" note is gone, and the proxy's DISCOVER now
  matches the reference (`0` for every measure, verified live on the upstream
  project). Recorded as a measured fact in `reference/claims.jsonl`
  (`measure-aggregator`) and referenced from the Excel-metadata page; the
  developer guide's aggregator row now says 0 with the multidimensional values
  called out. Known remaining difference: the reference also emits its hidden
  `__Default measure` (127, invisible to Excel); the proxy does not.

- **2026-09-29 — section D, oracle round: SQLMesh measured, not guessed.**
  `oracles/sqlmesh` is a uv-managed environment (SQLMesh 0.236.2, pinned by
  `uv.lock`; nothing in the product or CI depends on it) and
  `projects/upstream_marts_sqlmesh` is a real SQLMesh project with the same
  surface as `projects/upstream_marts` — dimensions, conformed fact, per-grain
  marts, metrics — materialising the same data (1,500 orders, revenue
  801,339.50, 264/33 mart rows) into DuckDB views under `upstream_marts.*` via
  `physical_schema_mapping`. Measured: model files carry `grain`/`columns`/
  `kind`/audits and metrics carry SQL expressions; there is **no built-in
  `relationships` audit** (that name is dbt's — a custom audit called from the
  model files is the convention); audit queries are not rewritten to physical
  tables and the logical views do not exist during apply, so the FK audit
  carries the declaration with `skip true` and the check runs in
  `qualify --contract`; `physical_schema_override` is deprecated. The
  generator (next) parses the files — no Python at generation time.

- **2026-09-29 — section D, the SQLMesh generator.**
  `mallard contract generate --from sqlmesh <project> [--overlay <file>]
  [--out <file>] [--check]` parses the checked-in files and maps what the
  metadata carries: `grain` (or a `unique_combination_of_columns` audit),
  relationships from the custom `relationships` audit calls, tables and
  columns, and `metrics/*.sql` (`SUM(column)` → sum; `COUNT(*)`/`COUNT(DISTINCT
  …)`; `MIN`/`MAX` with `valid_grain` from the source model's grain; a ratio of
  sums over one model → `ratio` with the expression normalised to
  source-neutral form). The sibling `contract.overlay.yaml` carries dimension
  ids/attributes/levels/date roles/display/time intelligence and per-measure
  extras; unknown models, missing grains, unclassifiable expressions and
  overlay typos refuse with the overlay named as the escape hatch. Output is
  validated like a hand-written file, `--check` compares instead of writing,
  and generation is byte-deterministic (`source_hash: fnv1a64:…` over the
  parsed inputs). The fixture generates
  `projects/upstream_marts_sqlmesh/contract.yaml`, whose semantics (grain,
  dimensions, relationships, measures, flag catalogue) equal the hand-written
  `contracts/upstream_marts/contract.yaml` — a test asserts both. CI wiring is
  deferred per the user's call.

- **2026-09-30 — section D, the generator on a real schema (TPC-H).**
  `projects/tpch_sqlmesh` runs the pipeline over TPC-H SF=0.1 (600,572 line
  items) generated locally by DuckDB's `tpch` extension: raw tables as
  EXTERNAL models (excluded from the contract), a conformed fact with a
  composite grain and five relationships, customer/part/supplier/nation/date
  dimensions, a monthly revenue mart, and nine metrics (sums, ratios of sums,
  `COUNT(*)` via the overlay's declared source, `COUNT(DISTINCT …)`, a
  `time_window` YTD). The contract generated on the first run;
  `qualify --contract` is READY in ~2.3 s at 600k rows; the served pivot
  matches direct SQL exactly (revenue by year, order counts, average discount,
  `Revenue YTD` = the `ytd_flag` slice). Findings: SQLMesh EXTERNAL models
  need a query body (a self-named `SELECT * FROM raw.<table>` works), and
  MallardCube serves bare table names, so materialised models must land in the
  connection's default schema (`main`); the overlay's declared-source path
  needed the model-name → table resolution this trial exposed (fixed). Next:
  the same run at SF=1 for the qualifier's scale behaviour, then explicit YAML
  (dbt deferred per the user).

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

- **2026-09-25 — grain checks.** For every additive measure, `qualify` compares
  its total with the sum over a relationship dimension's leaf members — the
  invariant every pivot subtotal relies on, and one that notices a multiplying
  join or a dropped key per measure. Time-windowed measures (YTD/QTD/MTD) look
  like `SUM(x)` but are not additive, so they are excluded from the invariant
  and instead require an oracle once an `oracles.json` exists. Verified: the
  demo model passes, and the oracle-coverage rule reports its uncovered
  windowed measures (captions are matched, not ids).

### Review round: two more fail-closed holes, and honest additivity (2026-09-25)

The review over the fail-closed + qualifier batch found two serving paths that
still bypassed the latch and one misclassification:

- **Drillthrough** was the only serving path that never observed the failure
  latch, so a failed query answered an empty rowset with a valid schema — the
  exact "plausible empty success" the section exists to prevent. It now clears
  before and faults after; the Contoso-style missing table the qualifier blocks
  for is the natural trigger. Fixed and tested with the `Failing` double.
- **The dimension cache** kept a dictionary built from a failed query, keyed on
  the dimension alone. One timeout during the first wide `MDSCHEMA_MEMBERS`
  would have left an empty hierarchy cached for the process lifetime, with no
  query left to fail and nothing to fault. Failed builds are not cached now, and
  entries are keyed by the data epoch so a reload or a late in-flight insert
  cannot be served.
- **`is_additive` accepted anything starting with `SUM(`,** so a ratio of sums
  was grain-checked (false-BLOCKING the thin-projection fixture, which is READY
  again) and exempted from oracle coverage. Additivity now means "exactly one
  additive aggregate": no operator at depth zero after it.

Smaller review items fixed: oracles resolve by id *or* caption as documented;
the additivity invariant runs over every relationship dimension, not just the
first; NULL fact keys are reported as their own finding; tolerances carry a 0.01
floor; an identifier containing a double quote is a configuration error rather
than a data defect.

**Qualifier batch completed 2026-09-28**: parent-child integrity checks
(duplicate keys, orphan parents, self-parents and a depth-capped cycle walk —
`parent_child_defects_are_blocked`), the machine-readable verdict
(`mallard qualify --json`, contract `mallardcube.qualify/1`: verdict, config,
reasons, notes, exit code; a test pins the shape), and the additivity
invariant computed in one SQL row (total, grouped sum, group count) instead of
pulling every group into Rust. Composite *relationship* keys have no
configuration surface yet (relationships are single-column), so there is
nothing to check there.

Parity cases for the `3a10f91` fixes landed with the mirror's values
(`members-foreign-property-faults`,
`mdschema-properties-foreign-property-faults`,
`catalogs-foreign-property-faults`, `drillthrough-empty-cube-faults` with the
double-space message); the catalogue is 67/67.

**Review round 2026-09-28** (qualifier batch) found and closed:

- The parent-child checks were unreachable for the documented configuration —
  the table was resolved from `fact_table`/date role, which that config does
  not set, while the serving path uses the relationship's `dim_table`. They
  resolve the same way now, an unresolved dimension reports "cannot check"
  instead of silently skipping, and a real-config test pins the wiring.
- `--json` returned before strict mode, so `--json --strict` reported READY
  for a project `--strict` rejects. Strict findings fold into the verdict
  before either output, and `ok` (verdict == READY) joins the JSON contract.
- The parent-child predicates now mirror the materializer exactly
  (string-cast joins, `''` and self-parents as roots — no false blocks), and
  the cycle walk is one O(rows) descent from the roots (unreachable nodes and
  a >64-level hierarchy block) instead of an O(N²) per-node walk.
- The grain invariant uses fixed-arity scalar calls: the multi-column
  `query_rows` derives its arity by scraping the statement, which a nested
  query breaks (a 2-column fact table blocked a valid project).
- The three foreign-property parity cases pin the refusal phrase
  (`fault_contains`), so a fault for the wrong reason no longer satisfies
  them; each case has its own source line.

Recorded, not fixed: a generation counter so a timed-out request's abandoned
worker cannot fault the next request; the pre-existing dimension-key check
still uses `query_rows` (its two-column select satisfies the scrape for
tables with at least two columns — a single-column table would mis-read), and
the statement-arity scraping itself is the underlying landmine.
