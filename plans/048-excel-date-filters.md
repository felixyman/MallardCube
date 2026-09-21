# Plan 048 — Excel date filters: metadata requirements + UI spike

Status: **in progress** — the SSAS-shaped date attribute hierarchy and the
Discover restriction handling landed; Excel still withholds Date Filters and
still refuses to add *any* hierarchy to a pivot built against the proxy.

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

## Next

1. **Make `DrilldownLevel` match SSAS** (likely the field-add blocker):
   include the input member(s) in the result, for flat dimensions too, and
   resolve the hierarchy segment — `[Date].[Date].All` must drill the date
   level, not the user hierarchy's years. Re-test the Excel field add after
   each: a fresh pivot cache + `CubeField.Orientation = 1` is a 30-second
   check, and the cache definition shows whether Excel accepted the field.
2. **Match the level enumeration.** The reference reports `count="2"` for a
   two-level hierarchy where Excel reports `count="0"` for ours. Compare the
   full `MDSCHEMA_LEVELS`/`MDSCHEMA_HIERARCHIES` rows (we are missing
   `DESCRIPTION`, the SQL column names, `LEVEL_MASTER_UNIQUE_NAME`;
   `LEVEL_CARDINALITY`/`HIERARCHY_CARDINALITY` are real here and 0 there;
   `DIMENSION_UNIQUE_SETTINGS` 0 vs 1; `GROUPING_BEHAVIOR` 0 vs 1;
   `HIERARCHY_ORDINAL` 0 vs per-hierarchy) and bisect until
   `memberValueDatatype="7"` appears in the cache definition.
3. Once Date Filters appear, capture the MDX they emit (`xmla-trace.jsonl`) and
   check it against the plan 046 date-window lowering.
4. Optional: `--auth-key` on windows-mcp + an `Authorization` header.

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
