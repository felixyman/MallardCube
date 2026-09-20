# Plan 039: Parent-child hierarchies

## Status

- **Priority**: P1 (biggest modeling gap for real Tabular models)
- **Effort**: M
- **Risk**: MEDIUM (startup ordering: augmentation must precede model build)
- **Depends on**: none
- **Category**: modeling

## Why this matters

Real BI models are full of self-referencing structures — org charts,
charts of accounts, product categories, BOMs. In Tabular these are
parent-child hierarchies: one table with `(key, parent_key)`. SSAS presents
them to MDX clients as synthetic levels (`Level 01..NN`, one per recursion
depth), with every member living at its own depth. MallardCube currently
supports only explicit multi-level hierarchies, so any model with an org
chart cannot be browsed/drilled.

## Design

**Materialize, don't recurse at query time.** Every downstream consumer
(discovery enumeration, MetaCount paths, SetMembers, drilldown group_level
swaps, compound-key unames) already works over plain `LevelDef.column`s.
So at config-load time we:

1. Open the project's DuckDB file directly (read schema, write augmentation
   columns — the project owns its DB; attached/read-only sources degrade to
   flat leaf lists with a warning, noted below).
2. Recursive CTE computes each member's `path` (pipe-joined ancestor keys)
   and `depth`:

   ```sql
   WITH RECURSIVE pc AS (
     SELECT k, CAST(k AS VARCHAR) AS path, 1 AS depth
       FROM t WHERE parent IS NULL
     UNION ALL
     SELECT c.k, p.path || '|' || CAST(c.k AS VARCHAR), p.depth + 1
       FROM t c JOIN pc p ON CAST(c.parent AS VARCHAR) = CAST(p.key AS VARCHAR)
   )
   ```

3. Materialize `_pc_depth` and `_pc_l1.._pc_lN` columns (ancestor key at
   depth i, NULL past the member's own depth) via
   `ALTER TABLE ADD COLUMN IF NOT EXISTS` + `UPDATE ... string_split(path)[i]`.
4. Synthesize `LevelDef`s (`Level 01..NN`) with live cardinalities and inject
   them into the model dimension.

Everything downstream then behaves exactly like an explicit hierarchy —
including compound-key unames (`&[A]&[B]`), parent links, TREE_OP, drilldown
ancestor chains, and set probes — because it *is* the same machinery.

### Config surface

```json
{ "id": "Employee",
  "physical_field": "employee_key",
  "parent_child": { "key_column": "employee_key", "parent_column": "parent_employee_key" },
  ... }
```

`hierarchy_levels` in config is ignored/overridden for PC dimensions;
levels come from data.

### Startup ordering

`ProxyProject::load/from_config` resolves `db_path` (existing helper) and
calls the augmentation *before* `build_semantic_model`, so the leaked
singleton is born complete. `Backend::init` afterwards opens the same file
and finds the materialized columns. Demo in-memory DB: not wired for v1
(file-backed projects only); AutoModel/converter detection is a follow-up.

## Out of scope (follow-ups)

- Self-referencing-FK auto-detection in converter/AutoModel
- Read-only/attached sources (would need on-query recursion or a cache table)
- Security filtering interplay with recursion
- Manual Excel E2E fixture project (wire tests cover semantics; a
  `--demo-pc` fixture would help manual checks)

## Test plan

- Unit: `prepare_parent_child(&conn, ...)` on a seeded temp table returns
  expected depths/cardinalities and materialized columns; idempotent rerun.
- Project load: config with `parent_child` yields a dimension whose levels
  match data (3 levels for the fixture org chart).
- Wire (temp-dir project + FileQueryBackend): HEAD/COUNT probes, TREE_OP
  matrix on PC members, drilldown root → child with compound unames and
  ancestor chains.
- Full suite stays green.

## Done criteria

- [x] Org-chart fixture: browse levels, SELF probes, drilldown all correct
- [x] Idempotent startup (restart doesn't duplicate/dirty augmentation; an
      existing materialization is read back, and read-only loads never write)
- [x] 410 tests green
