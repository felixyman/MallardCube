# Plan 053 — Big-data intake and the scale gate: storage contract, object store, capacity proof

## Status

- **Priority**: P2 (matters as soon as the data stops living on local NVMe)
- **Effort**: L
- **Risk**: MEDIUM (new storage backends; the validator must be conservative)
- **Depends on**: 050 (fixtures, bench), 051 (budgets, limits)
- **Related**: 054 (the engine owns storage; the proxy declares and validates)
- **Category**: intake / operations

## Why this matters

The proxy currently reads whatever DuckDB can read, and says nothing about
whether it is laid out for the query patterns Excel sends. Plan 050's fixture
is a best case: one 453 MB Parquet file on local NVMe. The realistic big-data
cases are worse:

- hundreds of thousands of small Parquet files on a network share;
- object storage (`s3://`) or table formats (Iceberg/Delta);
- no clustering key, so every pivot is a full scan with no row-group pruning;
- dimension tables large enough that a cold `SELECT DISTINCT` dictionary build
  (plan 031) is itself a network scan.

We measured the good case (23 ms p50 at 50M, plain; 4 ms with rollups). Nothing
tells a user why *their* model is slow, and nothing tells them how big a model
one node can carry. This plan adds the contract, the checks, and the proof.

## Design

### A. Storage contract + validator

A short, opinionated document (docs site → Deployment / Aggregations) and a
`mallard qualify-scale <config>` command that inspects the model and prints
actionable findings:

- **Clustering**: is the fact sorted or partitioned by the date column the
  time hierarchies use? (DuckDB prunes row groups only when the predicate
  matches the layout.) This is the single biggest plain-path lever.
- **File layout**: row-group sizes, file count/size distribution, Parquet
  statistics presence, tiny files, column count.
- **Dimensions**: cardinality per dimension (reuse the dictionary queries),
  flagging hierarchies past the plan 051 budget.
- **Warnings, not errors**, with the expected effect, e.g. "fact is not
  clustered by `date_key`: yearly drilldowns will scan the full table".
- The same checks run at startup in a debug log line so the shape of the data
  is visible in the log without a separate command.

### B. Object storage and table formats

The **engine owns storage**; the proxy owns the declaration and validation of
the semantic mapping on top of it. That keeps a second engine (plan 054) from
turning into a second intake implementation:

- DuckDB path: `s3://` via `httpfs` (endpoint override + credentials from the
  environment so MinIO works in tests), Iceberg/Delta scans via their
  extensions. **No internet at runtime**: extensions are vendored into the
  image and pointed at with `MALLARDCUBE_EXTENSION_DIR`; nothing may try to
  fetch an extension or reach a hosted service.
- Other engines later bring their own object-storage and table-format support;
  the proxy's job is config, validation and routing, not a parallel reader.
- Iceberg/Delta is the accepted "upstream materialisation" surface:
  configuration, credential passthrough, and a documented expectation that
  metadata/statistics reads are the new cold-start cost.
- Persist dimension dictionaries next to the aggregation artifacts so a restart
  does not re-scan a remote dimension table; invalidate on the same stamp as
  rollups.
- A MinIO-based fixture in the bench directory so this path is exercised, not
  just documented.

### C. The scale gate

Extend the harness so "big" is a routine, not an anecdote:

- `gen_bench_data.sh` profiles: `ROWS=500000000` and `1000000000` (documented
  disk/time cost), plus a **wide-dimension profile** (200k+ members) and a
  **many-small-files profile** (e.g. 50k files) to represent the bad cases.
- Gesture coverage in the workload: pivot cache build, single/two-field pivot,
  cross-tab, Top-N, value filter, expand, drill-through, refresh.
- Reported per run: p50/p95 per gesture, throughput, peak RSS, metadata p95,
  rollup build time, sidecar size. Gates pinned in the bench script (with
  `--report-only` for exploratory runs): e.g. gesture p95 ≤ 2 s at 500M with
  rollups, ≤ 1 GB RSS, metadata p95 ≤ 50 ms.
- Values stay verified against SQL ground truth; Excel-visible shapes stay
  verified against the mirror. The gate is the same arbiter as every other
  plan, just at scale.
- Run manually/nightly on the workstation or a cloud box, not in CI; commit
  the report (markdown) under `docs/` or `plans/` evidence sections.

### D. Capacity statement

From the gate results, write down the honest envelope in the docs: what one
node handles (rows, bytes, dimensions, concurrent users), where the proxies'
limits are (maintenance: budgets, timeouts), and when the answer is "materialise
upstream" (plan 052 D) or a different engine. Replace the vague "large models
work" claim with numbers and a date.

## Scope

**In:** layout contract + `qualify-scale`, httpfs/S3 (+ MinIO fixture),
Iceberg/Delta configuration, persisted dictionaries, the three bench profiles,
gates and the capacity doc.

**Out:** writing to object storage or table formats, a distributed query
engine, replication, multi-node deployment, arbitrary cloud provider plumbing.

## Done criteria

- `qualify-scale` on the repo fixtures produces the documented warnings (and
  none on the well-laid-out ones); a deliberately bad fixture (many tiny files,
  no clustering) is flagged with the expected message. Checks are engine-aware
  (capabilities and settings from plan 054), not DuckDB-hardcoded.
- The bench runs all three profiles end-to-end; a report for one ≥500M-row run
  is committed with the gate results.
- The MinIO fixture answers the same workload as the local fixture, with the
  measured delta recorded.
- The capacity statement exists in the docs and matches the committed report.

## STOP conditions

- If DuckDB's httpfs cannot be built/installed in the target environments,
  document the local/network-share path as supported and the object-store path
  as experimental rather than shipping a half-tested backend.
- If a 1B-row profile cannot be generated or run on the available hardware,
  publish the 500M numbers and mark 1B as untested — never extrapolate in docs.
