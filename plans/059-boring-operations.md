# Plan 059 — Boring operations: release, deploy, observe

## Status

- **Priority**: P1 (the cheapest credibility win, and half of it is independent)
- **Effort**: L
- **Risk**: LOW for the release half; MEDIUM for the deployment tooling
- **Depends on**: nothing for the release half; 057 for fingerprints/versions
- **Related**: 050 (benches), 051 (budgets, `/status`), 054 (settings surface)
- **Category**: operations / release engineering

## Why

The external review (2026-09-25) is blunt about this axis: releases are not
reliably publishable, deployment is one manually configured process, and
operations rely on `/status` and logs rather than metrics, readiness and
rollback. A staple must be **dull to install, upgrade and recover**. The release
half can start immediately and does not depend on any product plan.

## A. Release path (start now, in parallel)

- Audit and repair `.github/workflows/release.yml` (today it publishes through
  `softprops/action-gh-release`): verify the artifacts actually install, and
  make CI a **required predecessor** — a tag cannot publish without green tests.
- Publish Linux, macOS and Windows binaries plus a container image.
- Supply chain: SBOM (cyclonedx), signatures (cosign/sigstore), provenance
  (SLSA-style attestation), pinned third-party actions, dependency scanning.
- **Install from the published artifact** in CI (not from the repo): download,
  run `mallard --version`, start against the demo, hit `/status`.
- Publish upgrade and rollback instructions; support the current and previous
  release.

## B. Deployment

- **Immutable model releases**: `releases/<model>/<version>/` containing the
  contract, generated config and fallback files, plus the fingerprint from
  plan 057; a `current` pointer that only moves atomically.
- `mallard promote <version>` and `mallard rollback`: atomic swap, reload, and
  a health gate before the swap is reported successful.
- **Health vs readiness** endpoints (`/livez`, `/readyz`): readiness false while
  loading or reloading, true only when the model is serving.
- **Graceful drain**: stop accepting, finish in-flight requests, then exit; a
  wrapper/systemd `ExecStop` uses it.
- Per-model resource limits through the existing settings surface (memory,
  threads, pool, budgets); a systemd unit, a Helm chart, and an **air-gapped
  bundle** (vendored DuckDB extensions, no runtime downloads, no internet).
- One process per model stays the shape. A control-plane *service* is
  explicitly deferred: files + a CLI cover promotion and rollback until the
  number of models and teams justifies more.

## C. Observability

- Structured logs (JSON option) with a request id, echoed in the XMLA trace and
  in faults so a user report maps to a log line.
- A Prometheus `/metrics` endpoint: request latency histogram, error counts by
  class, in-flight and queued requests, result-cache entries/bytes/hit rate,
  data freshness and contract/model version.
- Keep `/status` for humans; `/metrics` is for machines.

## D. Capacity envelope and SLOs

Publish what the benches already measure, in operator language: supported fact
sizes, wide-dimension limits, concurrent-user expectations, memory and temp-disk
requirements, rollup behaviour, unsupported shapes, recommended topology. Then
state SLOs — p95 latency at a documented concurrency, an error budget, and the
reload window — and add a bench gate that fails when a release regresses them.

## Scope

**In**: release workflow repair + supply chain, install-from-artifact test,
immutable releases, promote/rollback CLI, health/readiness/drain, packaging
(systemd/Helm/air-gapped), metrics, structured logs, capacity + SLO docs.

**Out**: a registry/control-plane service, Kubernetes operators, multi-model
processes, autoscaling, hosted/cloud packaging.

## Done criteria

- A clean machine installs from published artifacts, deploys a model, promotes
  a new version, rolls back, and completes an N-1 upgrade — scripted and
  repeated in CI where possible.
- `/metrics` is scraped; readiness reflects load/reload state; drain finishes
  in-flight requests.
- The capacity envelope and SLOs are published, and a release fails when the
  bench gate regresses.

## STOP conditions

- If packaging needs a control-plane service to be usable, stop and simplify:
  the CLI + files path must stand alone first.
- No runtime extension downloads — if an extension is needed, it ships vendored
  and signed in the bundle.

### The audit job's findings (2026-09-26)

The dependency audit turned up five advisories on its first run. Four are fixed
in the lock: `quick-xml` 0.37.5 → 0.42.0 (RUSTSEC-2026-0195, a DoS in
`NsReader` — exactly this proxy's request parser, and there is no workaround
below 0.41), `rustls` 0.23.40 → 0.23.45 (RUSTSEC-2026-0285),
`anyhow` 1.0.102 → 1.0.104 (the unsound `downcast_mut` warning) and
`quinn-proto` 0.11.14 → 0.11.18.

The fifth, `rkyv` 0.7.46 (RUSTSEC-2026-0235), is **not compiled**: its only
reference in the lock is `rust_decimal`'s optional `rkyv` feature, and
`cargo tree --target all -i rkyv` prints nothing, so no target resolves it. The
audit job carries an explicit `ignore` for it with that rationale rather than a
silent pass.

**Retired 2026-09-26:** the duckdb 1.10505.0 update dropped `rust_decimal` (and
with it `rkyv`, `reqwest`, `quinn-proto`, `borsh`) from the dependency graph
entirely — `cargo tree --target all -i rkyv` now reports that no such package
exists — so the audit `ignore` was removed rather than kept as a blind spot.

The upgrade also taught us one thing worth keeping: `cargo update` with no
package filter re-resolved DuckDB and broke the build script — a security
refresh needs one package at a time, with the gates as the arbiter.
