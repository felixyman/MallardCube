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

3. **Still withheld.** With both metadata changes in place, Excel records
   `keyAttribute="1"` on `[Date].[Date]` (pivot cache definition) but resolves
   `memberValueDatatype="5"` — the `[Measures]` row's type — instead of the
   date dimension's 7. Excel's member-value lookup appears to take the first
   `PROPERTY_NAME=MEMBER_VALUE` row rather than matching the key attribute
   hierarchy. How SSAS shapes/orders that rowset is unknown here (no SSAS or
   Power BI XMLA endpoint available).

## Changes

- `src/mdx/semantic.rs`: `is_refresh_cube`.
- `src/execute/builders.rs`, `src/execute/runtime.rs`: `ddl_noop_response`
  returns the empty `ExecuteResponse` for no-op DDL.
- `src/xmla/discover/mdschema_properties.rs`: `DATA_TYPE` on `MEMBER_VALUE`
  rows (7 date role, 130 string keys, 5 measures).
- `src/xmla/discover/hierarchies.rs`: a date role's hierarchy reports
  `HIERARCHY_ORIGIN=4` (key attribute).

## Next

- Capture a real SSAS / Power BI XMLA
  `Discover(MDSCHEMA_PROPERTIES, PROPERTY_NAME=MEMBER_VALUE, PROPERTY_TYPE=5)`
  response to learn the expected row shape and order, then match it.
- Alternative: expose a true key attribute hierarchy for the date role
  ((All) + leaf, origin 6) beside the user hierarchy — the SSAS shape, at the
  cost of an extra field in Excel's list or renaming `[Date].[Date].[Year]`.
- Once Date Filters appear, capture the MDX they emit (`xmla-trace.jsonl`) and
  check it against the plan 046 date-window lowering.
