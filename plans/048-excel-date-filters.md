# Plan 048 — Excel date filters: metadata requirements + UI spike

Status: **in progress** — the pivot field-add regression is **fixed** (bisected to
`e9b7ab6` and repaired: duplicate standard member properties in the cellset).
Excel still withholds Date Filters: it does not write `memberValueDatatype` for
the date attribute hierarchy, so the field is not typed as a date.

## Why

Excel's own Date Filters (Year to Date, Between, Before/After, …) are the one
time-intelligence surface that cannot be reproduced from VBA or CUBE formulas:
`PivotFilters.Add2` is refused on OLAP pivots (1004) and the UI is the only
path. Capturing the MDX Excel sends for those filters needs a driver for the
Excel UI; plan 046 parked that behind a human at the VM.

## Harness (windows-mcp)

`cursortouch/windows-mcp` is wired into opencode as a remote MCP server
(`http://192.168.124.172:8081/mcp`), started by a per-user scheduled task on the
Windows VM so it runs in the **interactive session** (session 1). A session-0
server (what an ssh-spawned one gets) has no desktop: Snapshot returns
"No windows found" and UI automation is impossible. Tools used: `Snapshot`,
`Screenshot`, `Click`, `Move(drag=True)`, `Shortcut`, `PowerShell`, `App`.

Note: the endpoint is unauthenticated on the LAN and has full system access;
add `--auth-key` (and a header in the opencode config) before leaving it up.

## Findings (session 1 — pivot Refresh + first attempts)

1. **Pivot Refresh was broken.** Excel sends `REFRESH CUBE [<cube>]` before
   re-querying; the proxy faulted (`unsupported MDX: expected SELECT`), so the
   UI aborted with "The query did not run". The data is live (DuckDB), so the
   statement is a no-op: `is_refresh_cube` + `ddl_noop_response` answer with the
   empty success. Verified live — the UI refresh now succeeds and Excel
   re-reads the cube metadata and re-queries the pivot.

2. **Date Filters need three things** (MS docs: "OLE DB for OLAP properties
   used by Excel" and the OOXML `cacheHierarchy` note):
   - `MDSCHEMA_PROPERTIES.MEMBER_VALUE.DATA_TYPE` is a date type (7) for the
     key attribute of the Time dimension;
   - `HIERARCHY_ORIGIN` carries the key-attribute bit (MS-SSAS bitmask:
     1 = user-defined, 2 = attribute, 4 = key attribute) so Excel marks
     `keyAttribute="1"` and stores `memberValueDatatype`;
   - `MdpropMdxSubqueries` has the two lowest bits set (already advertised: 63).

3. **Still withheld.** Excel records `keyAttribute="1"` on `[Date].[Date]`
   (pivot cache definition) but resolves `memberValueDatatype="5"` — not the
   date dimension's 7 — and still shows no Date Filters.
   Note: the marking is only written on the `REFRESH CUBE` path (context-menu
   Refresh); `PivotTable.RefreshTable`, `Alt+F5` and `ExecuteMso("RefreshData")`
   re-read metadata but leave the cache definition unchanged.

## Findings (session 2 — reference oracle + SSAS shape)

The VM now runs a real **SSAS 2025 tabular** model (`MallardRef`) and Excel
connects to it, so every metadata question can be answered by comparison
(recipes in `.agents/skills/ssas-reference-oracle`).

4. **Verified date-filter rule (tabular reference).** Excel stores
   `memberValueDatatype` per **single-level attribute hierarchy**, read from the
   date level's `LEVEL_DBTYPE` (7 = `DBTYPE_DATE`; the same level also reports
   `MEMBER_VALUE DATA_TYPE=7`). The reference's cache says:
   `[DateDim].[FullDate] attribute="1" time="1" ... memberValueDatatype="7"`.
   A **user hierarchy** gets `time="1"` but no `memberValueDatatype` — so its
   levels never offer Date Filters. `keyAttribute="1"` is *not* what gates it:
   the reference's date column has no key bit, and marking ours with the key bit
   (origin 6) made Excel fall back to `memberValueDatatype="5"`.

5. **The SSAS shape is now emitted.** The date role's user hierarchy is
   `[Date].[Calendar]` (origin 1, time levels) and the full-date level is its
   own single-level attribute hierarchy `[Date].[Date]` (origin 2, `LEVEL_TYPE`
   0 on the date level, `LEVEL_DBTYPE` 7, `LEVEL_ORDERING_PROPERTY` /
   `LEVEL_ATTRIBUTE_HIERARCHY_NAME` set) — mirroring the reference's date
   column. `MEMBER_VALUE` rows are emitted per level per hierarchy
   (`(All)`=130, period levels=3, date levels=7).

6. **Discover restriction lists are honoured for `MDSCHEMA_PROPERTIES`.**
   Excel asks for one hierarchy's member properties at a time while it builds
   pivot cache fields; returning rows for every hierarchy is protocol-wrong and
   was corrupting the cache field. `Restrictions` now parses
   `CATALOG_NAME`/`CUBE_NAME`/`DIMENSION_UNIQUE_NAME`/`HIERARCHY_UNIQUE_NAME`/
   `LEVEL_UNIQUE_NAME`/`PROPERTY_NAME` and the properties rowsets filter on them.

7. **New blocker: Excel refuses to add *any* hierarchy to a pivot against the
   proxy.** Verified against a *fresh* reference cache in the same Excel
   instance: `CubeField.Orientation = 1` works there and fails here
   (0x800A03EC) for every hierarchy — including a plain flat dim — while
   `Orientation = 4` for measures works. The same refusal happens through the
   field-list checkbox (control: the same gesture adds a field to the reference
   pivot). No server request is sent for the failed adds, so Excel refuses from
   cached metadata. Two concrete MDX gaps line up with it:
   - `DrilldownLevel({[Category].[Category].[All]})` drops the **All root**;
     SSAS returns `All` + children (verified via ADODB against the reference).
   - `DrilldownLevel({[Date].[Date].[All]})` (the key attribute hierarchy)
     resolves to the **user hierarchy** and returns its years instead of dates —
     the engine resolves sets by level name and ignores the hierarchy segment.
   Excel queries exactly these shapes when a field is added.
   The pivot cache also still shows `count="0"` for our hierarchies where the
   reference has `count="2"`, and no `memberValueDatatype` at all — i.e. Excel
   is not enumerating our levels the way it enumerates the reference's.

## Findings (session 3 — the field-add regression, and where Date Filters stand)

8. **Excel refused to add any hierarchy field to a pivot against the proxy**
   (COM `CubeField.Orientation = 1` → 0x800A03EC, and the field-list checkbox
   too) while measures worked and the same calls worked against the reference.
   The last known-good state was the August test workbook, so this was a
   regression: `git bisect 84da600..master` (19 revisions) landed on
   **`e9b7ab6`** ("Excel hierarchy levels, expansion semantics, …", Sep 20).
   The field-add path fetches metadata and then runs the drilldown MDX; the
   requests and *all* non-member responses were byte-identical between the last
   good and first bad build. The cellset was not: `e9b7ab6` started emitting
   `MEMBER_CAPTION`, `MEMBER_UNIQUE_NAME`, `LEVEL_NUMBER` and
   `LEVEL_UNIQUE_NAME` as extra member-property elements. Those values already
   travel in the standard `Caption` / `UName` / `LNum` / `LName` tags and the
   duplicates are **not declared in `HierarchyInfo`**, so Excel rejects the
   cellset and aborts the field add. Fix: never emit them (`635e3f2`).
   Verified live: `[Category].[Category]`, `[Date].[Date]` (the key attribute
   hierarchy) and measures all add to a fresh pivot and it renders.
9. **`MEMBER_KEY` for compound members is the *leaf* key**, not the pipe path
   (`[DateDim].[Calendar].[Year].&[2020].&[1]` → `MEMBER_KEY=1`, verified
   against the reference). Reverted to that (`7223633`); it was not the
   field-add gate but it is the correct value.
10. **Date Filters still withheld.** With fields adding again, the date field
    can be inspected: Excel **auto-groups** it into Years/Quarters/Months/Days
    (it parses the captions as dates) but the Filter menu offers only
    Label/Value Filters — and the pivot cache for the hierarchy carries
    `attribute="1" time="1"` with **no `memberValueDatatype`**, where the
    reference writes `memberValueDatatype="7"`. Excel also **fails to save**
    ("Document not saved") any workbook whose pivot contains that field —
    another symptom of the same unresolved typing gap.
    Aligned so far: `LEVEL_TYPE` 0 on the date level, `LEVEL_DBTYPE` 7,
    `(All)` visible with DBTYPE 3, `LEVEL_ORDERING_PROPERTY` /
    `LEVEL_ATTRIBUTE_HIERARCHY_NAME`, `LEVEL_UNIQUE_SETTINGS` 0,
    `LEVEL_KEY_CARDINALITY` 1, hierarchy origin 2, `GROUPING_BEHAVIOR` 1,
    `STRUCTURE_TYPE`, `DIMENSION_UNIQUE_SETTINGS` 1, `HIERARCHY_ORDINAL`,
    `MEMBER_VALUE` per level (`(All)`=130, periods=3, date=7) with
    `PROPERTY_ORIGIN`/`PROPERTY_IS_VISIBLE`, and
    `MEASUREGROUP_DIMENSIONS.DIMENSION_GRANULARITY`. Cardinalities
    (`LEVEL_CARDINALITY`, `HIERARCHY_CARDINALITY`) were tried as 0 ("unknown",
    like the reference) and made no difference; reverted to the real values.

## Findings (session 4 — Date Filters close-out)

**Date Filters are exposed.** A fresh pivot on the key attribute hierarchy
(`[Date].[Full Date]`) against the proxy renders the dates and its Filter menu
offers **Date Filters...** — the same item the tabular reference offers on its
`[DateDim].[FullDate]` field — and the `Date Filter` dialog (equals / is before /
is after / … + calendar) opens. Four gates had to fall, in order:

1. **`DISCOVER_SCHEMA_ROWSETS` ignored the `SchemaName` restriction.** Excel
   asks for one rowset's entry to learn its restrictions; we answered with all
   63 rows, so Excel never learned we support `HIERARCHY_VISIBILITY` and took an
   older metadata path. It now returns just the requested rowset (verified
   against the reference: 1 row, same restriction list).
2. **`PREFERRED_QUERY_PATTERNS` was 0** (reference: 3 = `DrillDownMember` axes +
   implicit measures). With 0, Excel never asked for the key attribute's
   `MEMBER_VALUE` at all — the trace showed only `PROPERTY_TYPE=2` cell-property
   requests. With 3, Excel follows the reference's exact sequence:
   `DISCOVER_SCHEMA_ROWSETS(SchemaName=MDSCHEMA_HIERARCHIES)` →
   `MDSCHEMA_HIERARCHIES(HIERARCHY_VISIBILITY=3)` →
   `MDSCHEMA_PROPERTIES(PROPERTY_NAME=MEMBER_VALUE, PROPERTY_TYPE=5,
   PROPERTY_VISIBILITY=3)`.
3. **`MDSCHEMA_PROPERTIES` rows were in the wrong element order.** Excel reads
   the rowset positionally against the schema: our rows put `PROPERTY_NAME`
   before `PROPERTY_TYPE`, so it read `PROPERTY_TYPE`'s value (`5`) as
   `DATA_TYPE` and stamped `memberValueDatatype="5"` on *every* cache hierarchy.
   Rows now follow the reference's schema order (`… LEVEL_UNIQUE_NAME,
   PROPERTY_TYPE, PROPERTY_NAME, PROPERTY_CAPTION, DATA_TYPE, PROPERTY_ORIGIN,
   PROPERTY_IS_VISIBLE`), the schema carries the reference's full column list,
   `KEY0`/`NAME` carry the level's key type (`(All)`=3, int=20, date=7,
   string=130), and member-value rows are sorted by hierarchy with `[Measures]`
   last. The pivot cache then reads exactly like the reference's:
   `[Date].[Full Date] memberValueDatatype=7 time=1 attribute=1`, flat dims 130,
   user hierarchy and measures unmarked. (Excel writes the marking on the
   `REFRESH CUBE` path — the context-menu Refresh — and the workbook now saves
   normally too; the old "Document not saved" failure is gone.)
4. **The key attribute hierarchy did not execute as its own hierarchy.** Excel
   drags the field and sends
   `DrilldownLevel({[Date].[Full Date].[All]},,,INCLUDE_CALC_MEMBERS)`; the
   parser dropped the hierarchy part of the member reference, so the drill
   started at the *user* hierarchy's top level and the field rendered years
   (2020, 2021, …). `DrilldownTarget` now carries the hierarchy, a hierarchy
   that names a level selects that level, and the axis renders in the attribute
   hierarchy's own single-level namespace — `(All)` first, then
   `[Date].[Full Date].&[2020-01-01]` members with `LNum=1` and
   `PARENT_UNIQUE_NAME=[Date].[Full Date].[All]` — matching the reference's
   `[DateDim].[FullDate]` member shape. Excel places axis members by the field's
   hierarchy and level numbers; the user hierarchy's namespace (level 4) left
   the field empty.

Verified live (Book31): rows `2020-01-01 …`, Filter → Date Filters… →
`Date Filter (Full Date)`. The reference shows the same menu on the same field.
**Remaining:** applying a filter through that dialog is blocked by a VM input
quirk (the dialog accepts the first `SendKeys` burst after `SetForegroundWindow`
and ignores windows-mcp clicks afterwards), so Excel's date-filter MDX has not
been captured; the proxy's handling of it is unverified.

## Findings (session 4 — filters, sorting, top-N)

Excel's *value* idioms already lower correctly: `TopCount`/`BottomCount`,
`Order(…, DESC|ASC)`, and `Filter(set, [Measures].[X] > n)`. Two defects fell
out of exercising them:

1. **The plan key omitted the axis set op.** `plan_key` keyed `GroupBy` plans by
   measure/dims/levels/filters only, so the result cache served a `TopCount`
   answer for a `BottomCount`/`Order`/`Filter` query on the same dimension —
   `BottomCount(…, 2)` returned the cached top-3 members. The key now carries
   `setop=…` (`src/engine/normalize.rs`).
2. **Label filters were silently applied as value filters.** `axis_set_op`
   accepted *any* `Filter(set, <binary>)` with a numeric right-hand side, so
   `Filter(set, InStr(caption, "Bo") > 0)` became "measure > 0" and the axis came
   back unfiltered while Excel showed the filter as applied. A value filter now
   requires a **measure** on the left (`[Measures].[X] op n`), and any other
   `Filter` condition faults with an actionable message (`label filters … are
   not supported yet — use Keep Only Selected Items or a value filter`).

Excel's exact *label*-filter MDX is still uncaptured: the same modal-dialog
input limit that blocks the Date Filter dialog blocks Label/Value Filter and
Top 10 dialogs, and `PivotField.PivotFilters.Add2` refuses OLAP pivots
("Value does not fall within the expected range") — also confirmed for the
reference. The guard above makes the outcome loud rather than silently wrong.
Sorting (A→Z / Z→A) is client-side for OLAP and sends no query.

`scripts/bench-workload.jsonl` gained the two captured statements from this
session: the drill-through Excel sends on a value double-click
(`DRILLTHROUGH MAXROWS 1000 SELECT … WHERE (([Measures].[Revenue],[Date].[Full
Date].&[2020-01-01]))`) and the key-attribute-hierarchy field drill. The corpus
test now skips non-MDX statements (DRILLTHROUGH/DAX have their own paths).

## Changes

- `src/mdx/semantic.rs`: `is_refresh_cube`.
- `src/execute/builders.rs`, `src/execute/runtime.rs`: `ddl_noop_response`
  returns the empty `ExecuteResponse` for no-op DDL.
- `src/xmla/parser.rs`: `Restrictions` from the request's `RestrictionList`;
  `MdschemaProperties` carries them.
- `src/xmla/discover/mdschema_properties.rs`: `DATA_TYPE` on `MEMBER_VALUE`
  rows, per level per hierarchy (`(All)`=130, int=3, date=7, measures=5);
  rows filtered by the request's restrictions.
- `src/xmla/discover/hierarchies.rs`: date roles expose the key attribute
  hierarchy `[Date].[Date]` (origin 2) beside the user hierarchy
  `[Date].[Calendar]` (origin 1); `HIERARCHY_CAPTION` is the hierarchy name.
- `src/xmla/discover/levels.rs`: key hierarchy levels (origin 2, date level
  `LEVEL_TYPE=0`, `LEVEL_DBTYPE=7`, `LEVEL_ORDERING_PROPERTY`,
  `LEVEL_ATTRIBUTE_HIERARCHY_NAME`, `LEVEL_KEY_CARDINALITY=1`); user-hierarchy
  levels keep their time types and report `LEVEL_KEY_CARDINALITY=1`.
- `src/engine/model.rs`: `key_hierarchy_name` / `key_level` /
  `key_*_unique_name` helpers.
- `projects/project3/proxy-config.json`, `projects/upstream_marts/proxy-config.yaml`:
  date role user hierarchy renamed to `Calendar` (SSAS naming; required so the
  key hierarchy's unique name does not collide).
- Tests/corpus/docs: `[Date].[Date].<level>` references renamed to
  `[Date].[Calendar].<level>`.
- Session 4 (Date Filters close-out):
  - `src/xmla/schema_rowsets.rs`: `get_schemas_response` honours the
    `SchemaName` restriction (one rowset's entry, as the reference does).
  - `src/xmla/discover/cubes.rs`: `PREFERRED_QUERY_PATTERNS=3` (tabular).
  - `src/xmla/discover/mdschema_properties.rs`: reference element order + full
    column list; `KEY0`/`NAME` carry the level's key type; member-value rows
    sorted by hierarchy with `[Measures]` last and typed `WSTR`.
  - `src/mdx/parser.rs`: `DrilldownTarget` carries the hierarchy name.
  - `src/mdx/semantic.rs`: a hierarchy that names a level selects that level and
    renders flat; `SemanticQuery::key_hierarchy_view`.
  - `src/execute/axis_members.rs`: `apply_key_hierarchy_view` /
    `hierarchy_for_view` rewrite the axis into the attribute hierarchy's
    namespace and prepend its `(All)`.
  - `src/execute/render.rs`: applies the view in `build_drilldown`.

## Next

1. **Make Excel type the date field** — *done* (session 4). The cache reads
   `memberValueDatatype="7"` for `[Date].[Full Date]`, and the field renders the
   dates with a Date Filters menu. Remaining metadata deltas noted earlier
   (`DIMENSION_MASTER_NAME` vs `DIMENSION_MASTER_UNIQUE_NAME`,
   `INSTANCE_SELECTION`, empty GUIDs, empty `LEVEL_MASTER_UNIQUE_NAME` / SQL
   column-name fields) were not needed for it.
2. **Work around the save failure** — *moot*: with the metadata fixed the
   workbook saves normally, so `SaveAs(path, 51)` + reading
   `xl/pivotCache/pivotCacheDefinition1.xml` works.
3. **Then**: Date Filters → capture the MDX they emit (`xmla-trace.jsonl`) and
   check it against the plan 046 date-window lowering. **Open**: the menu and
   dialog are verified, but the dialog's OK could not be activated from the VM
   harness (input quirk, see session 4), so the emitted MDX is still uncaptured.
   Next attempt: drive the dialog with a single `SendKeys` burst after
   `SetForegroundWindow` (`{TAB}`/`{ENTER}`/`%o`) instead of windows-mcp clicks,
   or run the same gesture against the reference pump and diff.
4. **`DrilldownMember` drops the un-expanded members** — *fixed* (commits
   `647fe81`, `6a47054`). The query
   `CrossJoin(Hierarchize({DrilldownLevel({[Channel].[Channel].[All]})}),
   Hierarchize(DrilldownMember({{DrilldownLevel({[Date].[Calendar].[All]})}},
   {[Date].[Calendar].[Year].&[2022]})))` must return the *full input set* with
   2022 expanded in place — the reference SSAS returns 12 members (All, 2020,
   2021, **2022**, Q1–Q4, 2023, 2024, 2025, 2026); we returned 6 (All, 2022,
   its quarters) because the drill targets were applied as an axis *filter*.
   Now the multi-dimension path emits the un-expanded level-0 members with
   their per-other-slot aggregates (from a level-0 × level-0 grouping that
   ignores the drill filter), the single-dimension chain path keeps the whole
   input set too, and both paths report the **per-member** child count in
   `DisplayInfo`'s low bits (a quarter is 3, a year is 4 — the static
   whole-level cardinality, 132 and 44, corrupts Excel's hierarchy walk).
   Verified against the reference: same 12 members, same order, same
   `DisplayInfo` (3 / 0x20003 / 0x30004) and `CHILDREN_CARDINALITY`, same
   parents. New regression test asserts the whole member list.
5. **Open: Excel still does not *render* the expansion.** With the response now
   matching the reference member-for-member, Excel sends the right MDX
   (`DrilldownMember({{DrilldownLevel({All})}}, {[Date].[Calendar].[Year].&[2021]})`,
   visible in `xmla-trace.jsonl`), receives our cellset, and leaves the pivot
   showing only years — while the same UI gesture against the reference model
   (MallardRef) expands 2024 to its child "1" immediately.

   Verified by capturing the reference's **raw cellset XML** (`AdomdCommand.
   ExecuteXmlReader` on the reference, POSTed to a capture endpoint on the
   Linux side, saved as `/tmp/opencode/ref-cellset.xml`): our Axis0 members are
   now element-for-element identical (`UName,Caption,LName,LNum,DisplayInfo
   [,PARENT_UNIQUE_NAME]`; `DisplayInfo` 3 / 0x20003 / 0x30004; same LName /
   LNum / parents), and the slicer was aligned too (the reference keeps the
   axis dimension's *other* hierarchies — e.g. the key attribute hierarchy —
   with their default members; we dropped the whole dimension). None of it
   changed Excel's behaviour.

   Two decisive experiments (rewriting proxy in front of ours,
   `/tmp/opencode/rewrite-proxy.py`):
   - **Value marker**: rewriting every cell value in the expand response to
     `424242` leaves the pivot's numbers untouched → Excel never applies the
     response.
   - **SOAP fault**: answering the `DrilldownMember` request with a fault
     produces no error dialog and no status change → Excel is not even waiting
     on that response.

   So the blocker is *outside* the cellset body: it is about how Excel's pivot
   accepts the expand result at all. Also ruled out by experiment: tabular
   member unique names for below-root members, clearing `PARENT_SAME_AS_PREV`
   on the expanded member, zeroing `LEVEL_CARDINALITY`/`HIERARCHY_CARDINALITY`,
   and the All/child `LName`/`LNum`/`PARENT_UNIQUE_NAME` (all match the
   metadata and the reference).

   **The reference's own response was captured** with a byte-logging TCP relay
   on the VM (`C:\Users\Public\Documents\relay.ps1`: `localhost:2399 → 2383`,
   log `relay.bin`) and a Book13 pivot pointed at `Data Source=localhost:2399`
   with `CommandText=Model`. The expand works through the relay (2024 renders
   its child "1"), so a transparent relay is not the difference. Findings from
   the capture:
   - responses come back as `application/sx+xpress`; the *first* response of
     each session decodes with `ntdll!RtlDecompressBuffer` (format 3) at
     `ct+32`, length from the header field, into 652 bytes of SOAP XML;
   - the larger responses do not decode with plain XPRESS at any offset/length
     (framing or variant still unknown), so the exact bytes SSAS sends Excel
     for the expand are still unseen.

   **A real parity gap found (but not the blocker).** With the same
   level-scoped `DIMENSION PROPERTIES` Excel sends, the reference returns each
   *level-scoped* property **only on members of that level**: the `[Year]`-
   scoped properties appear on year members and are *absent* on quarter
   members; the unscoped `PARENT_UNIQUE_NAME` / `HIERARCHY_UNIQUE_NAME` appear
   on every member that has them (the `(All)` member carries only
   `HIERARCHY_UNIQUE_NAME`), and `PARENT_UNIQUE_NAME` is emitted twice on year
   members (unscoped + scoped). We emit the fixed set on *every* level.
   Replaying the reference's exact shape through the rewriting proxy still does
   not make Excel render the expansion — so this is parity work, not the fix.

   Metadata re-checked this round, all identical to the reference:
   `MdpropMdxSubqueries=63`, `MdpropMdxDrillFunctions=7`,
   `MdpropFlatteningSupport=1`, `MdpropNamedLevels=3`,
   `MdpropMdxDdlExtensions=23`, `MdpropMdxNamedSets=15`,
   `MdpropMdxSetFunctions=524287`, `HIERARCHY_ORIGIN` (user hierarchy 1, key
   attribute 2). Differences that turned out not to matter:
   `PREFERRED_QUERY_PATTERNS` (we 0, reference 3 — rewriting to 3 does not
   fix rendering); `MDSCHEMA_MEMBERS` (reference: `MEMBER_KEY=0` and
   `MEMBER_ORDINAL=0` for every member; we report `All` / 1..N);
   `MDSCHEMA_LEVELS` (`LEVEL_ATTRIBUTE_HIERARCHY_NAME` of the key hierarchy's
   `(All)` level: reference empty, we write `(All)`).

   **Resolved.** The blocker was `MDSCHEMA_PROPERTIES`. The reference answers a
   hierarchy-scoped request with, per level, `KEY0` and `MEMBER_VALUE` (plus
   `NAME` on the `(All)` level), all `PROPERTY_TYPE=5`, and no cell properties;
   we answered with a fabricated list of twelve standard member properties
   (type 1) plus the cell properties. That list made Excel ask ~38 dimension
   properties in every pivot query — the reference is asked for two — and left
   the expansion silently ignored.

   How it was found: the pump (`C:\inetpub\wwwroot\olap\msmdpump.dll`) was
   pointed at the reference over HTTPS (8443) and fronted by a logging relay
   that strips `X-Transport-Caps-Negotiation-Flags`, so the reference answers
   plain `text/xml` instead of `application/sx+xpress` and every exchange is
   readable in `C:\Users\Public\Documents\pumpproxy\NNN_*.xml`. Excel's
   requests to the two servers could then be diffed directly: to the reference
   `DIMENSION PROPERTIES PARENT_UNIQUE_NAME,HIERARCHY_UNIQUE_NAME` +
   `WHERE ([Measures].[Revenue])`; to us the 38-property list.

   With the reference shape (`src/xmla/discover/mdschema_properties.rs`,
   commit `72a2e59`): a fresh pivot's expand query is
   `DIMENSION PROPERTIES PARENT_UNIQUE_NAME,[Year]KEY0,[Year]MEMBER_VALUE,
   [Quarter]KEY0,[Quarter]MEMBER_VALUE … WHERE ([Measures].[Revenue])`, and
   **the expansion renders in Excel against our proxy** (2021 → 1,2,3,4 with
   all sibling years kept, verified live). The date-role `MEMBER_VALUE`
   `DATA_TYPE` (7) is preserved for the Date Filters work.
6. Optional: `--auth-key` on windows-mcp + an `Authorization` header.

## Harness notes

- The reusable recipes live in the repo skills:
  `.agents/skills/windows-mcp-desktop` (drive the VM's real desktop/Excel UI and
  COM in the interactive session) and `.agents/skills/ssas-reference-oracle`
  (deploy/query a real SSAS model, read Excel's cache definition, and the
  verified metadata rules).
- The pivot cache definition is the ground truth for what Excel decided:
  `SaveCopyAs` a copy and read `xl/pivotCache/pivotCacheDefinition1.xml`
  (`cacheHierarchy` attributes `time`, `keyAttribute`, `memberValueDatatype`).
- A **fresh** cache is required to see new metadata: Excel keys pivot caches by
  connection, so create a new connection (or workbook) rather than reusing one.
- Menu coordinates shift with the cursor position; verify with a screenshot
  before clicking (one stray click deleted the pivot table mid-spike; Ctrl+Z
  restores it, and the workbook was unsaved).
- A **modal Excel dialog blocks UI clicks** (a Document Recovery prompt made
  every click a no-op); check for a dialog before concluding a gesture failed.
- `pkill -f 'target/debug/[m]allard'` must not appear in the same shell command
  as the plain binary path, or it kills its own shell (self-kill trap).
