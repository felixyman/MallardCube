# Plan 031: Dimension metadata cache — eliminate N+1 metadata queries

## Status

- **Priority**: P2 (performance)
- **Effort**: M (the original "build at load time" design had to change — see below)
- **Risk**: LOW
- **Depends on**: none
- **Category**: performance
- **Status**: **DONE 2026-09-20**

## Why this mattered

Every MDSCHEMA_MEMBERS request and every drilldown axis recomputed the same
member facts from DuckDB:

| Site | Query | Frequency |
|---|---|---|
| All-member child count (`members.rs`) | `COUNT(DISTINCT <first level col>) FROM <dim_table>` | per dimension per request |
| Flat leaf members (`members.rs`) | `SELECT DISTINCT <physical_field> FROM <dim_table> ORDER BY …` | per flat dimension per request |
| Leveled members (`members.rs`) | one `SELECT DISTINCT <path> FROM <dim_table>` per hierarchy level | per leveled dimension per request (4 for the demo Date) |
| Drilldown child counts (`render.rs`) | `COUNT(DISTINCT <next level col>) FROM <dim_table> WHERE …` | **per axis member** (11 for a year axis) |
| Set-count probes (`plan.rs` MetaCount) | `COUNT(DISTINCT <path>) FROM <dim_table>` | per CUBESETCOUNT probe |

On a wide model a field-list open is tens of small queries; a drilldown adds one
per member. All of them are stable between data reloads.

## Design (as implemented)

`src/engine/dim_cache.rs`:

- `DimMembers { all_cardinality, leaf_values, level_paths }` — one dictionary
  per dimension. `level_paths` is per level a list of pipe-split paths, which
  also answers child counts by prefix (`DimMembers::child_count`) and set counts
  (`path_count`).
- `DimCache` hangs off `SemanticModel` (`model.dim_cache`), so a model and its
  data can never share entries across tests or projects.
- **Lazy, not load-time**: the demo database is created *after* the project
  loads, so a load-time build would have no data. The first lookup builds the
  dictionary; every later one is memory-only.
- **RLS bypass**: cached values are unfiltered, so filtered users (row-level
  security) keep the direct, role-scoped queries. The MetaCount executor arm
  does the same check explicitly, so counts cannot leak.
- **Reload**: `reload_data` clears the cache along with the result cache
  (plan 041), since paths and counts depend on the data.

Wired sites: `build_all_member_rows`, `build_leaf_member_rows`,
`build_level_member_rows` (`src/xmla/discover/members.rs`),
`member_child_count` (`src/execute/render.rs`), and the `MetaCount` arm of
`execute_plan_with_backend_and_context` (`src/engine/plan.rs`).

## Verification

- `flat_dimension_matches_direct_queries` / `leveled_dimension_matches_direct_queries`
  compare the dictionaries against the direct SQL (values, cardinalities, child
  counts, path counts).
- `second_lookup_is_served_from_memory` and `members_response_is_query_free_after_warmup`
  count backend calls: the first MDSCHEMA_MEMBERS builds the dictionaries, the
  second issues **zero** metadata queries and returns an identical rowset body.
- `clear_forces_a_rebuild` covers the reload path.
- Live: MDSCHEMA_MEMBERS (4,247 rows) and a year drilldown (8 cells, child
  counts 11/4/0) render identically on the rebuilt server; smoke 8/8.

## Notes / follow-ups

- Memory: paths duplicate the dim's values (Date: 11 + 44 + 132 + ~2 000). The
  plan's 50K-cardinality threshold is not needed yet; add it if a customer dim
  is huge.
- A per-role dictionary (caching RLS-filtered values) is a follow-up; filtered
  users are correct but pay the queries.
