# Plan 049 — Multi-axis and nested layouts: two engine bugs, verified against the reference

Status: findings verified 2026-09-22; fixed in `8ab03e3` (see Fixed / Still open).
Related: plan 048 (Excel metadata/date filters), the mirror model `MallardDemo`,
`.agents/skills/ssas-reference-oracle` (relay recipe).

## Why this exists

Plan 048 verified Excel's pivot surface one field at a time (one hierarchy on
Rows, no second field, no columns). A sweep of "more complex" layouts —
two fields in Rows, a hierarchy in Columns together with measures, measures on
Rows — against the mirror tabular model shows the proxy answers Excel's MDX
with a **different axis structure** and, for the full nested layout, an error
Excel cannot read.

Everything below was produced with Excel's *own* MDX, captured from the mirror
through a logging relay (`C:\Users\Public\Documents\parity\relay.py`,
`127.0.0.1:8095` → IIS `127.0.0.1:8090`), then replayed against the proxy and
against the mirror (ADOMD `ExecuteXmlReader`, raw cellset XML both sides).

## Finding 1 — `CrossJoin(<hierarchy>, {measures})` is split into separate axes

Excel sends, for Calendar on Columns and Revenue + Units in Values:

```mdx
SELECT NON EMPTY CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]},,,INCLUDE_CALC_MEMBERS)}),
                           {[Measures].[Revenue],[Measures].[Units]})
       DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME ON COLUMNS FROM [Model]
       CELL PROPERTIES VALUE, FORMAT_STRING, LANGUAGE, BACK_COLOR, FORE_COLOR, FONT_FLAGS
```

| engine | axes | members | cells |
|---|---|---|---|
| mirror (SSAS 2025 tabular) | 1 | 32 members = 16 tuples (`All` + 7 years × 2 measures) | 16 |
| proxy | 3 | a0 = `Revenue,Units`; a1 = `2020..2026`; a2 = four `All` members | 14 |

One-measure minimal repro (`... CrossJoin(Hierarchize({DrilldownLevel({[Date].[Calendar].[All]})}), {[Measures].[Revenue]}) ON COLUMNS`):
mirror answers one axis of 16 members (8 tuples), the proxy answers axis0 = the
8 years and axis1 = `Revenue` + four `All` members. A plain `{[Measures].[Revenue],[Measures].[Units]} ON COLUMNS`
(no CrossJoin) matches the reference (axis0 = the two measures).

So the split happens exactly when a hierarchy is cross-joined with the measures
set: the measures become their own axis instead of staying on the requested one.

## Finding 2 — `DrilldownMember(CrossJoin(...))` loses parents and siblings

Excel's MDX for Category + Channel in Rows:

```mdx
SELECT NON EMPTY Hierarchize(DrilldownMember(
         CrossJoin({[Category].[Category].[All],[Category].[Category].[Category].AllMembers},
                   {([Channel].[Channel].[All])}),
         [Category].[Category].[Category].AllMembers, [Channel].[Channel]))
       DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME ON COLUMNS FROM [Model]
```

- mirror: 101 tuples — `All`, then each category followed by its channels
  (`All`, Direct, Online, Retail, Wholesale where data exists).
- proxy: 20 tuples — one channel per category, no parent row per category and
  no channel `(All)` totals.

## Finding 3 — the full nested layout is unreadable for Excel

Pointing the mirror-built workbook's connection at the proxy and refreshing
(`relay-mirror-nested.xlsx`: Category→Channel on Rows, Calendar on Columns,
Revenue + Units in Values) fails in Excel:

> We couldn't get data from an external data source. … Errors in the rowset
> store. An unexpected error was detected. The schema of the store declared
> that a column can never contain null values, but a null value was specified.

The same workbook renders fine against the mirror. This is consistent with the
axis-structure mismatch (Excel indexes the axis it asked for; it gets three).

## Fix sketch

1. Keep the measures set on the axis it was requested on: when an axis
   expression is `CrossJoin(<set>, {measures})` (either order), the measures
   must become a member *dimension of that axis*, not a separate axis.
2. Implement `DrilldownMember(CrossJoin(parents, {(child.All)}), parents, child)`
   as: for every parent, emit the parent tuple followed by its child tuples
   (including the child `(All)`), i.e. the same shape the reference returns for
   Excel's nested-field MDX. Plan 048 fixed the single-hierarchy drilldown
   chain; this is the cross-joined variant.
3. Regression tests from the two statements above: member counts and captions
   per axis (101 tuples for Finding 2; 16 tuples for Finding 1), plus a cellset
   comparison against the mirror.

## Fixed (commit `8ab03e3`)

All three findings, verified against the mirror with Excel's own MDX:

| layout | mirror | proxy before | proxy now |
|---|---|---|---|
| `CrossJoin(Calendar, {Revenue,Units}) ON COLUMNS` | 1 axis, 32 members, 16 cells | 3 axes, 28 members, 14 cells | **identical** |
| `{[Measures].[Revenue]} ON COLUMNS, <nested rows> ON ROWS` | Axis1 = 82 members / 41 cells | 40 members / 20 cells | **identical** |
| `CrossJoin(Calendar, {Revenue}) ON COLUMNS, Category ON ROWS` | 16 × 21, 168 cells | 1 axis, 420 members | **identical** |
| `DrilldownMember(… {-{Baby}} …)` (collapse) | 40 tuples | 20 tuples | **identical** |

Mechanism:

- `src/mdx/frontend.rs`: `AxisSpec` (ordinal, dimensions, measures, slot
  order) + `axis_specs()`, threaded through `ParsedMdx` and `SemanticQuery`.
- `src/execute/render.rs`: `measure_dim_axis` / `merged_measure_axis` /
  `finish_dim_axis` keep the measures on their axis and place each axis at its
  requested ordinal; `build_cross_tab` renders a field-in-Columns × field-in-Rows
  pivot (one axis per edge, `(All)` first, cells row-major); `build_drilldown_member`
  emits the root tuple and each parent's `(parent, All)` aggregate; the
  multi-measure renderers emit the `(All)` member Excel reads as the Grand Total.
- `src/xmla/discover/{hierarchies,levels}.rs` now honour their restrictions
  (Excel asks for one hierarchy/level at a time while building a cache; both
  returned every row). `discover/mod.rs` carries the shared matcher.
- Regression tests: `crossjoined_measures_stay_on_the_requested_axis`,
  `two_axis_cross_tab_keeps_one_axis_per_edge`,
  `nested_rows_drilldown_returns_parents_and_totals`,
  `measures_axis_keeps_its_ordinal`.

## Refactor phases 1–3 (commits `b324ddb`, `23d97ba`, `1a0ea91`)

The three findings above were symptoms of the same architecture: the AST was
parsed, then flattened into a flag bag, a `SemanticQueryKind`, a `QueryPlan` and
a `QueryResult` shape, and each layer re-guessed what the one above knew. The
first three phases of the parser/render refactor are done:

1. **Oracle corpus** (`b324ddb`): five layouts Excel sends are pinned to the
   mirror's structures and values (nested rows, collapse, two measures on one
   axis, channel × category cross-tab, year × category cross-tab), plus the
   three-dimension case. It caught two more quirks on the way: cross-tab axes
   listed members in data order rather than the dimension's order, and cells
   for combinations with no data were sent as 0 where the reference omits them
   (sparse cell data — Excel shows a blank).
2. **Classification from the AST** (`23d97ba`): the `contains("CrossJoin(")`
   family, `detect_axis_dimension`, `parse_axis_level_members`,
   `parse_drilldown_targets`, the `AddCalculatedMembers` classification and
   `extract_drill_members` are AST questions now (`frontend::mentions_call`,
   `find_call`, `walk_expr`, `first_axis_dimension`, `with_body`). Only what MDX
   keeps opaque stays text-matched: the *quoted* `AS '…'` bodies and the
   `strtomember` probe, which is outside the supported syntax subset.
3. **N-dimension grouping** (`1a0ea91`): `QueryBackend::query_grouped_n` and
   `QueryResult::MultiGroupedN` carry N key columns; `build_multi_dim_pivot`
   renders one axis per requested edge with nested parents/children and
   coordinate-keyed, sparse cells. The three-dimension layout now matches the
   mirror (656 cells, identical tuples and values).

Remaining candidates (phase 4, only if a gesture forces it): fold
`SemanticQueryKind` into the axis plan so the probe compat layer is the only
shape-matching left, and retire the per-shape renderers behind one
coordinate-keyed renderer.

## Regression: "add to Values" broke in `b508cf9` (fixed)

Symptom: Excel could not add a measure to Values on a fresh pivot against the
proxy (`CubeFields(...).Orientation = 4` → `0x800A03EC`, silently reverted in
the UI), while the same workbook worked against the mirror.

Bisected to `b508cf9` (the cell-property advertisement). Two independent
metadata faults, both only visible once the list grew past eight properties:

- **Our MDSCHEMA_PROPERTIES row schema declared fields required that the rows
  omit.** The reference marks every field `minOccurs="0"`; ours had
  `CATALOG_NAME`/`CUBE_NAME`/`DIMENSION_UNIQUE_NAME`/`PROPERTY_NAME` required
  and several numeric types wrong (`PROPERTY_ORIGIN` int vs unsignedShort,
  `PROPERTY_CARDINALITY` unsignedInt vs string, …). Excel validates rows
  against that schema, rejected the rowset and aborted its metadata sweep
  before MDSCHEMA_MEASURES — so the pivot cache got no measure fields. The
  schema is now byte-for-byte the reference's.
- **The cellset did not declare the cell properties it advertises.** With
  `FONT_FLAGS`/`LANGUAGE` advertised, Excel asks for them in `CELL PROPERTIES`;
  our `CellInfo` lacked `<FontFlags/>`/`<Language/>`, so Excel rejected the
  cellset and silently reverted the field change. `render_cellset` now
  declares them (names and types as the reference does) whenever they are
  requested, and the cell-property rows match the reference's exactly
  (`PROPERTY_TYPE`, `PROPERTY_NAME`, `PROPERTY_CAPTION`, `DATA_TYPE`).

Also fixed in the same pass: the top-level `(All)` member on a plain
`DrilldownLevel({All})` axis (Excel's Grand Total row), verified against the
mirror (`All` + members, grand total 521,586,767). Set-op axes
(TopCount/Order/Filter) still omit it: summing the returned subset is not the
grand total, so that needs the plan to carry the real total.

## UI pass: the gestures COM cannot drive (2026-09-23)

Walking the menus in Excel found two real gaps behind the COM "filter object
created but nothing happens" symptom, both now fixed:

- **Top/Bottom N** (`7983b81`): Excel wraps the set in a subselect —
  `FROM (SELECT Generate(<set> AS [XL_Filter_Set_0], TopCount(Filter(Except(
  DrilldownLevel(<set>.Current AS [XL_Filter_HelperSet_0], …), …),
  Not IsEmpty(<measure>)), n, <measure>)) ON COLUMNS …)`. The proxy faulted on
  the `Filter(...)` as an unsupported label filter (the `Not IsEmpty` predicate
  was not recognised) and Excel silently showed everything. The predicate is
  now treated as a "has data" test, the subselect's `TopCount` reaches the plan
  as `AxisSetOp::TopCountFilter` (keeps the top n *in the outer axis's order*,
  which is what the reference returns), and the proxy answers Excel's exact MDX
  member-for-member like the mirror: `All, Furniture, Garden, Health, Jewelry,
  Shoes` with `(All)` = 137,116,126.
- **Value Filters** (`708b840`): the condition arrives parenthesised —
  `Filter(<set>, ([Measures].[Revenue]>26000000))` — which the parser models as
  a one-item tuple, so the set-op extractor never saw the comparison. Verified
  against the mirror: same twelve members in the same order, and Excel renders
  them with the subset total.

Verified at parity with the mirror rather than fixed:

- **Page/report filter**: the dropdown lists only `(All)` against the proxy —
  and against the mirror too (Excel asks `{AddCalculatedMembers({[Territory].
  [Territory].[(All)].Members})}`, which both engines answer with one member).
- **Drill-through on a cross-tab**: Excel's nested-tuple statement
  (`DRILLTHROUGH MAXROWS 1000 … WHERE ((([Measures].[Revenue],
  [Category].[Category].&[Automotive]),[Channel].[Channel].&[Direct]))`) is
  lowered correctly; the empty sheet for that cell just reflects the demo
  pairing each category with a single channel (Automotive → Retail returns
  957 rows).

### Small items

- **Subtotals**: the `Subtotal "<field>"` menu (cell context menu on a row
  label) adds the `"<Category> Total"` rows on the proxy — Excel takes them
  from the `(All)` child members we emit for the nested field, no re-query.
  Client-side, working.
- **Show Values As**: `% of grand total` works and renders identically on both
  engines (already covered by the layout sweep). The *parent* variants cannot
  be set through the object model for OLAP (`DataFields(1).Calculation = 11`
  leaves the grid blank — identically on the mirror, so it is parity, not a
  proxy gap); the parent cell values it would use are the ones the subtotal
  test just proved Excel reads correctly.
- **Docs deployment**: cross-page links were broken once deployed — a relative
  `./slug/` from `/installation/` resolves against the page URL, not the docs
  root. Thirteen links are now `../slug/`, `site/scripts/check-links.mjs`
  verifies every built `href`/`src` (249 references) as part of
  `npm run build`, and CI builds the site on pull requests.

## Still open

- **Set-op axes and the `(All)` member** (see above): the reference keeps
  `(All)` on `TopCount`/`Order`/`Filter` axes too, with the grand total.
- **Slicer axis hierarchy list**: the reference lists the axis dimensions'
  other hierarchies (e.g. `[Date].[Full Date]` beside `[Date].[Calendar]`);
  the proxy lists only the query's non-axis dimensions. Invisible in Excel.
- **Slicer axis hierarchy list**: the reference includes the axis dimensions'
  other hierarchies (e.g. `[Date].[Full Date]` beside `[Date].[Calendar]`);
  the proxy lists only the query's non-axis dimensions (plan 048 note).
- **Excel cannot add a measure to Values via COM against a fresh proxy pivot**
  (`Orientation = 4` → `0x800A03EC`, silently in the field list too) while the
  mirror accepts it; an older proxy workbook still renders values. Restriction
  handling in `MDSCHEMA_HIERARCHIES`/`MDSCHEMA_LEVELS` was fixed while chasing
  it, but the cause is still open — compare the pivot cache definition of a
  fresh proxy pivot (no `[Measures]` cache field) with the mirror's.


## Harness notes (verified)

- **Logging relay on the VM** — `python C:\Users\Public\Documents\parity\relay.py`
  listens on `127.0.0.1:8095` and forwards to `127.0.0.1:8090`, writing
  `NNNN_req.xml` / `NNNN_resp.xml` into `C:\Users\Public\Documents\relay\`.
  Point an Excel connection at `http://127.0.0.1:8095/OLAP/msmdpump.dll` to read
  exactly what Excel sends to the reference. This is how Excel's nested-field
  MDX was captured.
- **Excel cannot add a measure to Values via COM against the proxy** in a fresh
  pivot: `CubeFields('[Measures].[Revenue]').Orientation = 4` (and
  `AddDataField`, and the field-list context menu) fail with `0x800A03EC`
  (silently, no dialog) while the same call succeeds against the mirror.
  Not caused by the MDSCHEMA_PROPERTIES change — the binary from `b508cf9`
  reproduces it — and not a server-side problem: an older proxy workbook
  (`date-cache-ours6.xlsx`) still renders values. Workaround for sweeps: build
  the layout against the mirror (through the relay) and compare MDX directly
  against both engines, as done here.
- Raw cellset XML is the only fair comparison: ADOMD's reader flattens
  (extra header rows, hierarchy columns) and the reference's `<Value>` elements
  are unformatted doubles (`7.7866061E7`) where the proxy writes plain
  integers — compare numerically.
