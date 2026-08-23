# Plan 038: Set expressions and calculated-member evaluation

## Status

- **Priority**: P1 (completes the CUBE-function surface)
- **Effort**: M
- **Risk**: MEDIUM (classification order changes)
- **Depends on**: none
- **Category**: compatibility
- **Status**: DONE (CUBECOUNT client-side quirk tracked separately)

## Why this matters

Excel implements `CUBESET` validation and `CUBECOUNT` with two MDX probe
shapes (captured verbatim from a live session, xmla-trace):

```mdx
-- set-shape probe
SELECT {HEAD([Date].[Date].[Year].Members,1)} ON 0 FROM [Sales]
  CELL PROPERTIES CELL_ORDINAL
-- count probe
WITH MEMBER [Measures].[XL_SD] AS 'COUNT([Date].[Date].[Year].Members)'
  SELECT {[Measures].[XL_SD]} ON 0 FROM [Sales] CELL PROPERTIES VALUE
```

Today neither evaluates correctly: `HEAD(...)` is ignored (the whole level is
returned), and the `WITH MEMBER ... COUNT(...)` calculated member falls into
the measure-by-category path and returns an unrelated aggregate grid. Excel
therefore shows `#VALUE` for every CUBECOUNT/CUBESETCOUNT cell.

Every fix so far has been shape-by-shape (`DrilldownLevel`, `.Members`,
cChildren special cases). This plan adds the missing structural piece: a small
set-expression IR plus a numeric evaluator for the one function Excel probes
with (`COUNT`).

## Design

### Set expression IR (parser.rs)

```rust
pub enum SetExpr {
    /// `[Dim].[Hier].[Level].Members` (level None = leaf/physical grain).
    LevelMembers { dim: String, level: Option<String> },
    /// `[Dim].[Hier].[(All)].Members` — children of All.
    AllMembers { dim: String },
    Head(Box<SetExpr>, usize),
    Tail(Box<SetExpr>, usize),
}
```

Parsed by scanning the SELECT axis clause for `{ HEAD( … , n ) }`,
`{ TAIL( … , n ) }`, or a bare `{ [….Members] }` set. Stored on `ParsedMdx`
as `axis_set_expr: Option<SetExpr>`.

### Calculated member (parser.rs)

```rust
pub struct CalculatedCount {
    pub member_name: String,   // e.g. "XL_SD"
    pub set: SetExpr,
}
```

Scans `WITH MEMBER [Measures].[name] AS '…'` where the expression body is
exactly `COUNT(<set>)`. Stored as `calculated_counts: Vec<CalculatedCount>`.

### Classification (semantic.rs)

Two new `SemanticQueryKind` variants, matched before existing patterns:

- `SetMemberProbe(SetExpr)` — axis carries a wrapped/bare set expression.
- `SetCountProbe { name: String, set: SetExpr }` — a calculated count member
  appears in the SELECT axis.

### Execution

- **SetCountProbe** → metadata count, no fact table involved:
  `SELECT COUNT(DISTINCT <level column>) FROM <dim_table>`. New
  `QueryPlan::MetaCount { dim, level }` handled in the SQL emitter and plan
  executor; result renders through the existing scalar path with a
  `[Measures].[<name>]` member on Axis0.
- **SetMemberProbe** → reuse the single-dimension GroupBy plan at the parsed
  level (same as DrilldownCategories), then prune rows in the renderer:
  Head(n) keeps first n, Tail(n) keeps last n. Members render level-aware
  (qualified unames), reusing the drilldown conventions.

### Out of scope (follow-ups)

- `Filter`/`TopCount` over resolved sets (axis_set_op already covers some)
- Range sets `{a : b}`, `Descendants(...)`, `Generate`, string-expression
  calculated members
- Non-COUNT numeric aggregates inside WITH MEMBER

## Test plan

- Parser unit tests: HEAD/TAIL/bare-Members extraction, calculated-count
  extraction (single quotes, escaped &amp;)
- Semantic tests: both kinds classify before legacy patterns
- Render tests under project3: HEAD(years,1) → exactly 1 tuple `[Year].&[2020]`;
  COUNT(years)=11; TAIL(years,2) → 2029+2030
- Wire battery: replay the exact Excel probe MDX shapes
- Excel MCP: `=CUBECOUNT(CUBESET(conn,"[Date].[Date].[Year].Members"))` → 11

## Done criteria

- [ ] Both probe shapes return correct results (unit + wire)
- [ ] Real Excel: CUBECOUNT(CUBESET(...)) resolves to 11, no #VALUE
- [ ] Existing suite stays green (371 passing)
