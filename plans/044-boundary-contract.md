# Plan 044: Boundary contract — a protocol adapter, not a semantic layer

## Status

- **Priority**: P1 (strategic — shapes every future feature decision)
- **Effort**: M (docs + a `qualify` gate + converter policy; no runtime rewrite)
- **Risk**: LOW (no behaviour change beyond a new strict gate)
- **Depends on**: none
- **Category**: architecture / positioning
- **Status**: TODO

## Why this matters

The modern data stack splits cleanly in two:

- **Definitions and materialisation live upstream** — sqlmesh/dbt models, metrics in
  code, tested, versioned, lineage, environments — materialised into DuckDB,
  Parquet, or an attached database.
- **Excel-facing serving lives at the edge** — the XMLA/MDX wire protocol, Excel's
  axis conventions, metadata rowsets, caching.

Serious teams will not put measure logic in a proxy config: it duplicates the
semantic layer, it is untestable next to the warehouse, and it makes the proxy
the owner of business definitions. The drift is already visible in this repo:

- the converter lowers DAX into `sql_fallback/` SQL;
- the config can express filtered and derived measures;
- every future feature request (calculation groups, DAX time intelligence,
  measure-of-measure arithmetic) pulls in the same direction.

Left unbounded, the next such request wins by default and the project becomes a
*worse* semantic layer instead of a *good* adapter.

External validation for the adapter position: Cube — the leading open-source
semantic layer — shipped an XMLA/MDX API for Excel precisely because modern
warehouses don't speak MDX, so Excel users lost direct connectivity after
migrations. That is the niche, and it is a protocol niche, not a modelling one.

## The contract

**MallardCube owns:**

- the XMLA/MDX wire protocol (SOAP envelopes, `DISCOVER`/`MDSCHEMA_*` rowsets,
  cellset XML, sessions, `CELL PROPERTIES`);
- Excel shape fidelity (axis conventions, drill/collapse, `DISPLAY_INFO`,
  member properties, `MEMBER_KEY`, `DRILLTHROUGH`);
- delivery mechanics (connection pool, result cache, hot reload, ops endpoints);
- a **mechanical projection** of an upstream model onto SQL.

**MallardCube does not own:**

- measure definitions beyond `SUM(column)` and plain SQL over exposed tables;
- DAX evaluation, calculation groups, measure dependency graphs;
- time-intelligence functions (the date-flag contract stays: flags are
  *columns*, computed upstream);
- join semantics beyond declared relationships;
- pre-aggregation semantics (the rollup sidecar is a cache, never a definition).

The `proxy-config` becomes a **projection** of what the transformation layer
already materialised — derived where possible, authored only as a last resort.

## Invariants and how they are enforced

| # | Invariant | Enforcement |
|---|---|---|
| 1 | Measures are `SUM(col)` or plain SQL over exposed tables | `qualify --strict` reports and fails on fallback SQL and non-additive measures |
| 2 | No DAX | converter stops growing lowering patterns; complex measures become a "define upstream" checklist; `sql_fallback` frozen as labelled bridge code (bug fixes only) |
| 3 | Flags and definitions live upstream | the date-flag contract is the worked example: the proxy reads `ytd_flag`, it never computes YTD |
| 4 | Rollups are a cache, never semantics | they already accept only `SUM(col)`; docs say "if the upstream pre-aggregates, disable `MALLARDCUBE_AGG_CACHE`" |
| 5 | Feature intake rule | any request that would add measure-level logic is answered with "do it upstream" plus a recipe, and recorded in the invariants doc so contributors see it before proposing a DAX engine |

## The upstream pairing: sqlmesh + DuckDB

| Semantic-layer capability | sqlmesh + DuckDB | MallardCube |
|---|---|---|
| Metric definitions (named, versioned, tested, lineage) | yes (metrics/models, column-level lineage, environments) | consumes the result |
| Joins / star-schema correctness | yes (models + audits) | declared relationships only |
| Grain changes, fan-out, non-additive metrics | yes, materialised at a declared grain | must not re-aggregate beyond that grain |
| Pre-aggregations | yes (incremental models) | optional rollup cache otherwise |
| Storage and execution | yes (DuckDB/Parquet) | — |
| Query API | no | XMLA/MDX (the product) |
| Query-time caching | no | result cache |
| Excel fidelity (axes, drill, metadata) | no | yes |
| RLS **policy** | partial (roles as data / per-role views) | enforcement: trusted header → SQL predicates |
| Column-level masking, multi-tenant context | no | no (table-level OLS only) |

In short: **sqlmesh + DuckDB is the semantic layer minus the serving runtime**;
MallardCube is deliberately that runtime, for Excel only.

### Gaps and how they are handled

1. **Non-additive metrics** (distinct counts, medians, percentiles, ratios):
   materialise a mart at the reporting grain. They cannot be re-aggregated
   (median of medians ≠ median), so pivot flexibility is pinned to that grain —
   a modelling decision the data team makes explicitly.
2. **Fan-out / grain safety at query time**: nothing stops a projection from
   joining a mart to a finer-grain table and double counting. Add a `qualify`
   check that warns when a fact is joined to a dimension that is not unique on
   the join key.
3. **RLS policy vs enforcement**: the policy is upstream (role → SQL predicate),
   enforcement is at the edge (trusted header → injected predicate). Column-level
   masking is the known gap; per-role views upstream are the documented answer.
4. **No automatic pre-aggregation matching**: marts are explicit. Our rollup
   cache remains the fallback for stacks without marts.
5. **sqlmesh metrics are not consumed directly**: the contract is the
   materialised tables, not sqlmesh metadata. A derived-projection target can be
   revisited if sqlmesh exposes a stable metric API.

### Recipe (what to prescribe)

1. Define metrics in sqlmesh with dimensions and filters; materialise:
   - a conformed fact with **additive columns** (0/1 status flags, `sum_x`,
     `count_x`);
   - **per-grain aggregate marts** for cumulative and non-additive metrics;
   - conformed dimensions with keys/labels and hierarchies.
2. Point MallardCube at the marts (DuckDB file, Parquet, or an attached DB);
   every proxy measure is `SUM(col)`; rollups off when marts already
   pre-aggregate.
3. Enforce with `qualify --strict` plus the cardinality check.

## Work items

| # | Item | Effort | Notes |
|---|---|---|---|
| 1 | Design-invariants + "what belongs upstream" docs (generic orders example), README positioning line, product-summary update | S | Foundation; cite the external validation |
| 2 | `qualify --strict`: semantic-creep report (fallback SQL, non-additive measures) + relationship cardinality warning | S/M | The mechanism that stops drift |
| 3 | Converter policy: freeze DAX lowering, emit a per-measure "define upstream" checklist, label fallbacks as bridge code | S/M | Conversion report becomes a migration plan |
| 4 | (Optional, later) Postgres-wire attach target (Cube Core OSS / Trino / Postgres marts) — spike first: dialect fit, pushdown, per-user context, DRILLTHROUGH | M | Only if a target is actually needed |
| 5 | (Later) Auto date hierarchies from date/time columns (converter + AutoModel): Year/Quarter/Month/Day, Year/Week, slicer-friendly Year and Quarter-of-year | M | Model *shape* fidelity, not semantics |

Items 1–2 are the anti-drift core and can land independently of the rest.

## Non-goals

No DAX engine, no calculation groups, no time-intelligence functions, no
measure-of-measure evaluation beyond plain SQL, no multi-dialect SQL emitter, no
attempt to replicate a headless-BI feature set, no growth of `sql_fallback`
beyond bug fixes.

## Parked

- **Cube MDX API as a reference oracle** for the deep-expansion Excel crash:
  requires Cube Cloud Enterprise (not available). OSS Cube Core has no XMLA
  endpoint, so the crash investigation stays parked until a real SSAS / Power BI
  XMLA endpoint is available.
- **Cube Core (OSS) attach target**: viable without Enterprise (its SQL API is
  open source, exposed on port 15432) but not required by this plan; it stays a
  later, optional target.

## Verification

- Docs: the invariants and the upstream contract are readable and use only a
  generic example.
- `qualify --strict` returns non-zero for a project with fallback SQL or
  non-additive measures, and zero for a thin `SUM(col)` projection; the
  cardinality warning fires on a many-sided join.
- Converter: a model with complex DAX produces a checklist entry per measure and
  no new lowering patterns.
- No runtime behaviour changes: existing tests stay green.

## Done criteria

- [ ] "Design invariants" and "what belongs upstream" docs published; README and
      product summary state the position.
- [ ] `qualify --strict` implemented, tested, and documented (CI-usable).
- [ ] Converter emits the "define upstream" checklist; DAX lowering frozen.
- [ ] Plan index updated; the feature-intake rule recorded.
