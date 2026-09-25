# Plan 061 — Intake paths and the proof journeys

## Status

- **Priority**: P2 (nothing above becomes true at scale without this)
- **Effort**: L
- **Risk**: MEDIUM (live attachment changes engine semantics; keep it bounded)
- **Depends on**: 053 (storage contract, object store), 057 (serving contract)
- **Related**: 052 (aggregates), 045 (intake fidelity), 050 (benches)
- **Category**: product / integration

## Why

The contract (057) says what to serve; this plan says where the data comes from
and proves the journeys end to end. Two constraints shape it: on-prem teams will
not move their warehouse into DuckDB on day one, and the proxy must not become a
federation engine. So intake is a small number of **supported, boring paths**
plus scripted journeys that run from a clean machine.

## A. Intake paths

Pick the load-bearing ones, in this order:

1. **Scheduled publish** of governed marts to a DuckDB file (or Parquet on
   object storage) — same trust boundary as today, the default for SQLMesh and
   dbt users. Reuse plan 053's storage contract.
2. **Read-only attach** to PostgreSQL or SQL Server as a **bounded mode**:
   vendored and signed scanner extensions, read-only credentials, documented
   performance and failure semantics, no federation, fail closed when the
   upstream is unreachable.
3. **SQLMesh publishing workflow** (the upstream default) and a dbt
   materialisation flow, each producing the file that intake (1) serves.

Live attach is a separate mode, not a config flag: it changes what a "query"
costs and what a failure means.

## B. Proof journeys

Each journey is a scripted, reproducible run — clean machine, no maintainer VM:

1. **SSAS migration**: inventory → convert → load → qualify → Excel validation
   → rollback.
2. **SQLMesh**: model change → audit → publish → generate contract (057) →
   deploy (059) → Excel validation.
3. **dbt**: upstream model change → generate contract → deploy, with no manual
   proxy edits.
4. **Air-gapped on-prem**: offline install → OIDC/reverse proxy → data refresh
   → restore → upgrade.

Run what can run in CI (1–3 up to the Excel step, with the Excel step recorded
as a fixture), and keep 4 as a documented drill with a checklist and a captured
transcript.

## C. Ecosystem surface

- Publish the adapter/extension interface — the engine seam: one SQL emitter
  module, capabilities as data, no speculative traits (the existing 044/054
  decision), written for a data-platform team to extend without owning a second
  product.
- Keep the contract JSON Schema stable, with semantic versioning and a public
  deprecation policy (shared with 057).
- Contributor guide, local development container, reference stacks, and a
  lightweight RFC process for compatibility changes.

## Scope

**In**: publish path, bounded attach mode, the four journeys, the interface and
contributor surface.

**Out**: federation, distributed execution, native adapters for BI tools,
hosted pipelines, writing back to warehouses.

## Done criteria

- An ordinary upstream model change (SQLMesh and dbt) reaches Excel with no
  manual proxy configuration, no business-logic duplication, and no contact
  with the maintainers.
- The four journeys are reproducible from a clean machine, with the Excel step
  captured as a fixture and the rest scripted.
- The extension interface and the deprecation policy are published, and a
  reference adapter exists outside the core repo's own fixtures.

## STOP conditions

- If attach mode cannot fail closed or needs runtime extension downloads, stop
  and keep it out of the supported set.
- No live federation, ever, without a new plan that says how failures and costs
  are bounded.
