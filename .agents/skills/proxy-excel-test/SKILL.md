---
name: proxy-excel-test
description: >
  End-to-end test the MallardCube SSAS proxy (Excel/XMLA frontend for DuckDB)
  using the real Microsoft MSOLAP client through the Excel MCP server, plus
  deterministic curl smoke assertions. Use when asked to test/verify the proxy,
  reproduce a proxy bug from an Excel/MDX report, validate a proxy change, or
  run an end-to-end "does it work from Excel" check. Triggers: proxy, XMLA,
  MDX, CUBEVALUE, CUBEMEMBER, SSAS, MallardCube, test the proxy, verify proxy.
---

# Proxy Excel E2E Test Skill

Test the MallardCube proxy from a **real Microsoft client** (Excel/MSOLAP), not
just curl. Two layers, in order of cheapness:

1. **Server-side smoke** (`scripts/proxy-smoke.sh`) — deterministic curl
   assertions against the running proxy. Always run this first.
2. **Excel MCP CUBE-function test** — drive Excel over the MCP server to write
   live `CUBEMEMBER`/`CUBEVALUE`/`CUBESET` formulas and read back resolved
   values. This is the ground truth for "does it work from Excel".

## Environment facts (do not rediscover these)

- The proxy runs **on Linux** (`cargo run` / `target/debug/mallard`), and by
  default binds **`127.0.0.1:8080` only**.
- **HARD REQUIREMENT: always start it with `BIND_ADDRESS=0.0.0.0:8080` and
  `MALLARDCUBE_ALLOW_ANONYMOUS=1`** (the proxy refuses a non-loopback bind without
  auth otherwise). The
  Windows VM/Excel cannot reach `127.0.0.1`; it reaches the Linux host on the
  LAN/VPN interface. If you start without it, curl smoke still passes locally
  but every Excel MCP test fails with a connection hang/timeout. Verify with
  `ss -tlnp | grep 8080` → must show `0.0.0.0:8080`, not `127.0.0.1:8080`.
- Excel + the MCP server run **on a Windows machine** that reaches the Linux
  host as `http://ssasproxy:8080/xmla` (hosts entry). If you only get a
  hostname/IP from the user, use it in the connection string.
- Default project `projects/project3`: catalog `SALES_ANALYTICS`, cube `Sales`,
  5 dims (Category/Territory/Channel/Segment/Date), 6 measures (Revenue, Units,
  RevenueYTD, Revenue Prior Year, RevenueQTD, RevenueMTD).
- Proven MSOLAP connection string (the one that works):
  `Provider=MSOLAP.8;Integrated Security=SSPI;Persist Security Info=True;Data Source=http://ssasproxy:8080/xmla;Update Isolation Level=2;Initial Catalog=SALES_ANALYTICS`
- Existing test workbook on the Windows box:
  `C:\Users\Public\Documents\manual-mallardcube-proxy-test.xlsm` — already has a
  connection named `http___ssasproxy_8080_xmla SALES_ANALYTICS Sales` and a
  PivotTable occupying `$A$1:$C$18` on Sheet1. **Reuse it**; write test formulas
  in cells outside `$A$1:$C$18` (e.g. `E1`, `E2`, …).

## Part A — server-side smoke (do this first)

```bash
# start the proxy CORRECTLY (0.0.0.0 so the Windows VM can reach it) + run smoke
cd /home/felix/code/MallardCube
bash scripts/proxy-smoke.sh serve     # starts proxy in background, waits for ready
bash scripts/proxy-smoke.sh           # deterministic assertions (exits non-zero on fail)
```

`scripts/proxy-smoke.sh` asserts known demo values and exits non-zero on
failure. Reference values (demo data is deterministically seeded, so these are
stable across runs):

| Check | MDX | Expected |
|---|---|---|
| Total Revenue | `SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]` | `521586767` |
| Units total | `SELECT {[Measures].[Units]} ON COLUMNS FROM [Sales]` | `4931640` |
| Electronics (tuple on axis) | `SELECT {([Measures].[Revenue],[Category].[Category].&[Electronics])} ON 0 FROM [Sales]` | `24719896` |
| Category count | `SELECT [Category].[Category].Members ON ROWS, {[Measures].[Revenue]} ON COLUMNS FROM [Sales]` | 20 categories, Electronics row = `24719896` |

Do NOT assert exact YTD/QTD/MTD values — they are computed against
`CURRENT_DATE` and change daily. Assert a strict subset of the total instead
(e.g. YTD < total revenue).

## Part B — Excel MCP CUBE-function test (the method that works)

The MCP server exposes `excel-mcp` tools. In the Code Mode runtime, call them
through `execute`:

```js
// 1. Open the workbook (reuse the existing one with the live connection)
const open = await tools["excel-mcp"].file({
  action: "open",
  path: "C:\\Users\\Public\\Documents\\manual-mallardcube-proxy-test.xlsm",
  show: false, timeout_seconds: 120
});
const sid = open.session_id;

// 2. Write CUBE formulas to a clear cell (NOT inside a PivotTable).
//    Use US comma separators in .Formula (not the localized ";")
await tools["excel-mcp"].range({
  action: "set-formulas", session_id: sid, sheet_name: "Sheet1",
  range_address: "E2",
  formulas: [[
    "=CUBEVALUE(\"http___ssasproxy_8080_xmla SALES_ANALYTICS Sales\"," +
    "\"[Measures].[Revenue]\",\"[Category].[Category].&[Electronics]\")"
  ]]
});

// 3. Force recalculation (CUBE functions resolve asynchronously)
await tools["excel-mcp"].calculation_mode({
  action: "calculate", session_id: sid, scope: "workbook"
});

// 4. Read the resolved value
const got = await tools["excel-mcp"].range({
  action: "get-values", session_id: sid, sheet_name: "Sheet1",
  range_address: "E2"
});
// got.values === [[24719896]]  ← the resolved numeric result

// 5. Always close (save=false for throwaway cells; save=true to persist proof)
await tools["excel-mcp"].file({ action: "close", session_id: sid, save: false });
```

Member/measure reference syntax:

- Measure: `"[Measures].[Revenue]"`, `"[Measures].[RevenueYTD]"`
- Member: `"[Category].[Category].&[Electronics]"` (leaf), `"[Date].[Calendar].[Year].&[2023]"` (level-qualified)
- Set: `"[Category].[Category].Members"` (for `CUBESET`), then `CUBECOUNT(conn, cellref)`
- `CUBEMEMBER` returns the member caption; `CUBEVALUE` returns the numeric cell.

If a cell reads back `#GETTING_DATA...` or empty, recalc again and re-read
(one retry is usually enough).

## Part C — ADOMD via VBA (arbitrary MDX, when CUBE functions aren't enough)

For a specific MDX shape Excel won't generate (e.g. reproduce a parser bug),
drive MDX directly through the same MSOLAP provider from VBA:

```vba
Public Sub RunMdx()
    Dim c As Object
    Set c = CreateObject("ADOMD.Cellset")
    c.Open "SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales]", _
      "Provider=MSOLAP.8;Integrated Security=SSPI;Data Source=http://ssasproxy:8080/xmla;Initial Catalog=SALES_ANALYTICS"
    Sheet1.Range("B1").Value = c(0).Value   ' cell value
    c.Close                                  ' REQUIRED (see gotchas)
End Sub
```

**Preconditions**: Excel Trust Center → Macro Settings → "Trust access to the
VBA project object model" (registry `AccessVBOM=1`) AND macros enabled.
The MCP `vba` tool needs `.xlsm` (not `.xlsx`).

## Part D — verify via the trace (ground truth)

With `XMLA_TRACE=1`, every request/response is NDJSON in `xmla-trace.jsonl`:

```python
import json, re
for line in open('xmla-trace.jsonl'):
    d = json.loads(line)
    if d.get('request_kind') == 'ExecuteStatement':
        m = re.search(r'<Statement[^>]*>(.*?)</Statement>', d['request_xml'], re.S)
        vals = re.findall(r'<Value[^>]*>([^<]*)</Value>', d['response_xml'])
        print(m.group(1), '->', vals)   # exact MDX the client sent, and the result
```

This is how you confirm (a) which MDX shape the client actually emitted and
(b) whether the proxy returned the right number. When a CUBE-function test
gives a surprising result, check the trace before concluding anything.

## Part E — UI-level checks (windows-mcp, session 1)

Some Excel surfaces exist only in the UI: pivot **Date Filters**, context menus,
dialogs, field-list drags. For those use the `windows` MCP server against the
**visible** Excel in session 1 — full recipe in the `windows-mcp-desktop` skill:

- select a pivot cell via COM, `Shift+F10`, screenshot the menu, click **Filter**,
  read the submenu;
- add fields to an OLAP pivot with `CubeField.Orientation` (1=row, 4=data);
  `AddFields` is refused for OLAP;
- read what Excel decided from the saved pivot cache definition
  (`xl/pivotCache/pivotCacheDefinition1.xml`, `cacheHierarchy` attributes
  `time`, `attribute`, `keyAttribute`, `memberValueDatatype`).

**Pivot refresh sends `REFRESH CUBE [<cube>]`.** The context-menu Refresh does;
`PivotTable.RefreshTable`, `Alt+F5` and `ExecuteMso("RefreshData")` re-read
metadata but do not. The proxy answers REFRESH CUBE as a no-op (empty success,
commit 95c4e37) — if a UI refresh fails with "The query did not run", grep the
trace for `REFRESH CUBE`. The cache definition is only rewritten on that path,
so use the context-menu Refresh when comparing cache definitions.

**Date Filters are not offered by the proxy yet.** Excel only offers them on a
field whose `memberValueDatatype` is 7, which it reads per *single-level
attribute hierarchy* from that level's `MEMBER_VALUE` DATA_TYPE. The proxy's date
role is one multi-level hierarchy, so Excel finds no such level → label/value
filters only. Full rule + reference-engine recipe: `ssas-reference-oracle`
skill and `plans/048-excel-date-filters.md`.

## Gotchas (learned the hard way — do not repeat)

- **`Connections.Add2` via the excel-mcp hangs forever.** It pops a modal
  "Import Data"/security dialog the MCP cannot click, and the fixed 120s MCP
  timeout kills the session. Don't create connections through the excel-mcp.
  Via the `windows` MCP's PowerShell/COM in session 1 it *does* work when
  `CommandType = 1` (xlCmdCube) — see `windows-mcp-desktop`; with `3` Excel
  reports "can't find table".
- **Do not chain `file open` + `vba` in one `execute` block.** The `vba` call
  fails generically ("An error occurred invoking 'vba'"). Open first, then
  `vba import` in a separate `execute` call, then `vba run` in a third.
- **ADOMD `Open` without `c.Close` hangs the MCP.** The lingering MSOLAP session
  stops Excel from reporting idle → 120s timeout kills the session. Always
  `c.Close` (and `ThisWorkbook.Save` if you want values to survive the kill).
- **The `connection` MCP tool's `list`/`create` currently throw
  `E_INVALIDARG`/`ArgumentException`.** Don't rely on it; discover the
  connection name via VBA (`ActiveWorkbook.Connections(i).Name`) or just use the
  known name above.
- **Crashed sessions leave file locks.** Use `file list` to find stale sessions
  and close them; `file open` fails with "already open in another session" until
  you do.
- **`window show` is transient** — Excel re-hides after each MCP op. Don't rely
  on a visible window for hidden automation.
- **Cell value reads can be `null` if the formula didn't resolve.** Always
  `calculation_mode calculate` before `get-values`, and verify no `cellErrors`.

## Known proxy bugs (verified 2026-08-23 — assert the right things)

- **None of the previously listed axis/slicer bugs reproduce.** Verified against
  the current build:
  - Time-intelligence measures on an axis return the flag-filtered subset:
    `SELECT {[Measures].[RevenueYTD]} ON COLUMNS FROM [Sales]` → `35714238`
    (a strict subset of the `521586767` total). QTD → `10718936`, MTD → `2647528`.
  - Bare single-member `WHERE (member)` slicers are honoured:
    `SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Category].[Category].[Electronics])`
    → `24719896` (same as the tuple-on-axis form). Level-qualified slices work
    too: `WHERE ([Date].[Calendar].[Year].&[2024])` must equal the raw-SQL
    expectation for that year — the demo data is bounded to "today", so the
    number shifts over time. Compare against
    `SELECT SUM(revenue) FROM sales_fact f JOIN date_dim d ON f.date_key = d.date_key WHERE d.year = 2024`
    instead of trusting a stale constant.
  - Tuple-on-axis, grouped-by-category, level-qualified CUBEVALUE members and
    the full discover handshake all work.
- **Excel caches CUBE function results client-side.** If you write a formula
  whose text already exists in the workbook (or was queried earlier in the
  Excel process), recalculation may not hit the server and `get-values` returns
  a stale cached number. Before concluding anything, confirm the request in
  `xmla-trace.jsonl`; if it's missing, write the formula to an unused cell with
  different arguments to force a fresh query.

## Test workflow for a proxy change

1. Make the change, `cargo build`.
2. Kill any running proxy, restart with `XMLA_TRACE=1` (Part A command).
3. `bash scripts/proxy-smoke.sh` — must be green.
4. `cargo test --lib` — repo unit tests.
5. Excel CUBE-function check (Part B) for any behaviour you changed.
6. Inspect `xmla-trace.jsonl` (Part D) to confirm the client's MDX and result.
7. Report actual vs expected with the exact MDX + value as evidence.
