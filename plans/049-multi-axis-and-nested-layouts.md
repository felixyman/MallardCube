# Plan 049 — Multi-axis and nested layouts: two engine bugs, verified against the reference

Status: findings verified 2026-09-22; fixes not started.
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
