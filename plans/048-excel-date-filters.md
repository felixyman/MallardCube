# Plan 048 — Excel date filters: metadata requirements + UI spike

Status: **in progress** — pivot Refresh fixed; Excel still withholds Date Filters.

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
`Screenshot`, `Click`, `Shortcut`, `PowerShell`, `App`.

Note: the endpoint is unauthenticated on the LAN and has full system access;
add `--auth-key` (and a header in the opencode config) before leaving it up.

## Findings

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
   date dimension's 7 — and still shows no Date Filters. Attempts, each with a
   `REFRESH CUBE`-triggered cache rewrite and the saved pivot cache definition
   as evidence:
   - emitting the time dimension's MEMBER_VALUE rows first (in case Excel takes
     the first row) — still 5;
   - marking the date role's key level with `LEVEL_ORIGIN=4` and the day level
     with the spec value `LEVEL_TYPE=116` (`0x74`) — both genuine fixes, kept —
     still 5;
   - removing the `[Measures]` MEMBER_VALUE row — the key-attribute marking
     disappeared entirely, so Excel expects that row to be present.
   The remaining 5-valued candidates are the measure `DATA_TYPE` and the
   MeasuresLevel `LEVEL_DBTYPE` (both correctly 5 for doubles), i.e. Excel may
   not read the MEMBER_VALUE rowset the way the documentation implies. Only a
   reference response from a real engine can settle it.
   Note: the marking is only written on the `REFRESH CUBE` path (context-menu
   Refresh); `PivotTable.RefreshTable`, `Alt+F5` and `ExecuteMso("RefreshData")`
   re-read metadata but leave the cache definition unchanged.

## Changes

- `src/mdx/semantic.rs`: `is_refresh_cube`.
- `src/execute/builders.rs`, `src/execute/runtime.rs`: `ddl_noop_response`
  returns the empty `ExecuteResponse` for no-op DDL.
- `src/xmla/discover/mdschema_properties.rs`: `DATA_TYPE` on `MEMBER_VALUE`
  rows (7 date role, 130 string keys, 5 measures).
- `src/xmla/discover/hierarchies.rs`: a date role's hierarchy reports
  `HIERARCHY_ORIGIN=4` (key attribute).
- `src/xmla/discover/levels.rs`: a date role's key level reports
  `LEVEL_ORIGIN=4`, and the day level reports `LEVEL_TYPE=116`
  (`MDLEVEL_TYPE_TIME_DAYS`; the previous 96 was not a valid time type).

## Next

- **Get a reference engine.** A real SSAS (SQL Server Developer Edition is
  free) or a Power BI Desktop local AS instance answers this in one capture:
  run `Discover(MDSCHEMA_PROPERTIES, PROPERTY_NAME=MEMBER_VALUE,
  PROPERTY_TYPE=5)` against a cube with a date dimension and compare the rows
  (shape, order, DATA_TYPE) with ours. That also settles every other Excel
  metadata question.
- If a key attribute hierarchy turns out to be required: expose one for the
  date role ((All) + leaf, origin 6) beside the user hierarchy — the SSAS
  shape, at the cost of an extra field in Excel's list or renaming
  `[Date].[Date].[Year]`.
- Once Date Filters appear, capture the MDX they emit (`xmla-trace.jsonl`) and
  check it against the plan 046 date-window lowering.

## Harness notes

- The reusable recipes live in the repo skills:
  `.agents/skills/windows-mcp-desktop` (drive the VM's real desktop/Excel UI and
  COM in the interactive session) and `.agents/skills/ssas-reference-oracle`
  (deploy/query a real SSAS model, read Excel's cache definition, and the
  verified metadata rules below).
- The pivot cache definition is the ground truth for what Excel decided:
  `SaveCopyAs` a copy and read `xl/pivotCache/pivotCacheDefinition1.xml`
  (`cacheHierarchy` attributes `time`, `keyAttribute`, `memberValueDatatype`).
- Menu coordinates shift with the cursor position; verify with a screenshot
  before clicking (one stray click deleted the pivot table mid-spike; Ctrl+Z
  restores it, and the workbook was unsaved).
