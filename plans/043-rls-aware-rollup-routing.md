# Plan 043: RLS-aware aggregation routing

## Status

- **Priority**: P1 (performance for secured deployments)
- **Effort**: M
- **Risk**: MEDIUM (security-adjacent — a rollup must never bypass RLS)
- **Depends on**: none (031/041 made the surrounding code cache/reload aware)
- **Category**: performance / security
- **Status**: **DONE 2026-09-20**

## Why this mattered

Rollups are built from the full fact table, so the original routing rule was
"any active role filter disables rollups" (`aggregation_safe`). That is safe but
it means every row-level-security user — the norm in enterprise deployments —
full-scans the fact for every pivot. On the 100M-row benchmark the difference is
~4 req/s versus ~300 req/s for the same workload.

## Design (implemented)

`src/engine/sql.rs`:

- **`rewrite_predicate_for_rollup`** maps a role-filter SQL fragment onto the
  rollup when every column reference is carried there with the same values:
  - flat-dimension values, bare (`territory = 'North'`) or aliased
    (`f.territory = 'North'`);
  - date level columns the rollup stores (`_date.year = 2024` → `year = 2024`;
    a deeper level than the rollup stores is refused);
  - expressions over those columns (`RIGHT(territory, 3) = 'rth'`), since the
    function is evaluated on identical values.
  - Refused (→ fact path): measures and fact columns that were not rolled up,
    other aliases/tables, quoted identifiers, subqueries.
  - Dimension aliases match case-insensitively because the join builder
    lowercases them (`_Date` → `_date`).
- **`rollup_role_predicates`** collects the predicates for every filtered table
  (fact tables and relationship dim tables) and returns `None` if any of them is
  not expressible.
- **`route_plan`** now returns `(rollup, rewritten_predicates)`; the predicates
  are ANDed into the rollup query by `agg_where`, so the rollup is filtered
  exactly like the fact path.

## Verification

- `rollup_predicate_rewrite_is_conservative`: 5 expressible forms (bare, `f.`,
  multi-condition, function, date level) and 8 refused forms (measure, unrolled
  column, deeper date level, dim column not carried, unknown alias, quoted
  identifier, subquery, `f.region`).
- `rls_routing_requires_expressible_predicates`: admin routes with no extra
  predicates; `territory = 'North'` routes and the generated rollup SQL carries
  the predicate; `revenue > 1000` keeps the fact path.
- `rollup_with_role_predicate_matches_the_fact` (aggregate tests): builds a real
  rollup from the demo data and asserts the role-filtered rollup total equals
  the fact total for `territory = 'North'` and a two-condition predicate.
- 439 tests green, clippy/fmt clean; 100M-row benchmark in `docs/SCALING.md`.

## Notes / follow-ups

- Predicates that reference unrolled fact columns still keep the fact path. If
  that shows up in real deployments, per-role rollups (or rolling up the
  referenced column) are the next lever.
- The demo generator correlates categorical draws (`North` only ever has
  `Direct`), which makes some predicate combinations empty — worth fixing for
  demo credibility (same care as the date-bounded fix).
