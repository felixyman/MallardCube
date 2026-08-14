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
- **HARD REQUIREMENT: always start it with `BIND_ADDRESS=0.0.0.0:8080`.** The
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

Do NOT assert YTD/QTD/MTD values — they are computed against `CURRENT_DATE`
and change daily (and are currently broken, see "Known proxy bugs").

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
- Member: `"[Category].[Category].&[Electronics]"` (leaf), `"[Date].[Date].[Year].&[2023]"` (level-qualified)
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

## Gotchas (learned the hard way — do not repeat)

- **`Connections.Add2` and native connection creation hang forever.** They pop a
  modal "Import Data"/security dialog the MCP cannot click, and the fixed 120s
  MCP timeout kills the session. Do NOT create connections programmatically.
  Reuse the existing workbook's connection (Part B) or use ADOMD (Part C).
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

## Known proxy bugs (current state — assert the right things)

- **Time-intelligence measures on an axis return full totals.** `SELECT {[Measures].[RevenueYTD]} ON COLUMNS FROM [Sales]` → `521586767` (should be a
  2026-only subset). Grouped by Year, every year shows its full-year sum. This
  is the important bug — it's the MDX shape Excel emits for a YTD measure.
- **Bare single-member `WHERE (member)` slicers are ignored.** `SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Sales] WHERE ([Category].[Category].[Electronics])` → full total. (Excel's CUBE functions don't emit this form;
  they use tuple-on-axis, which works.)
- **Works correctly**: tuple-on-axis `SELECT {([Measures].[X],[Dim].[Hier].&[key])} ON 0 FROM [cube]`, grouped-by-category, and the full discover
  handshake (all 15+ rowsets answer in ~40ms).

## Test workflow for a proxy change

1. Make the change, `cargo build`.
2. Kill any running proxy, restart with `XMLA_TRACE=1` (Part A command).
3. `bash scripts/proxy-smoke.sh` — must be green.
4. `cargo test --lib` — repo unit tests.
5. Excel CUBE-function check (Part B) for any behaviour you changed.
6. Inspect `xmla-trace.jsonl` (Part D) to confirm the client's MDX and result.
7. Report actual vs expected with the exact MDX + value as evidence.
