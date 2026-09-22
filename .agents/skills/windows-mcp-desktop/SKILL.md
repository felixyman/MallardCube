---
name: windows-mcp-desktop
description: >
  Drive the Windows VM's real desktop and Excel UI (menus, dialogs, pivot field
  lists, screenshots) through the windows-mcp MCP server, plus COM/PowerShell
  automation in the interactive session. Use when a task needs UI interaction
  that COM/VBA cannot do (pivot Date Filters, context menus, modal dialogs),
  when the excel-mcp's hidden session-0 Excel is not enough, or when you need a
  shell on the VM. Triggers: windows-mcp, desktop, UI automation, screenshot,
  click, Excel UI, pivot menu, session 1, interactive session.
---

# Windows desktop + Excel UI automation (windows-mcp)

The `windows` MCP server (cursortouch/windows-mcp) runs **on the Windows VM** and
exposes UI Automation: screenshots, element trees, clicks, keys, plus a full
PowerShell tool. It is the only way to reach Excel UI that COM/VBA refuses
(e.g. pivot **Date Filters**), and it doubles as a shell into the VM.

## Environment facts (do not rediscover)

- Server: **streamable HTTP** at `http://192.168.124.172:8081/mcp`, wired into
  opencode as `mcp.windows` (`type: remote`) in `~/.config/opencode/opencode.jsonc`.
  Started by the per-user scheduled task `windows-mcp-server` on the VM.
- **It must run in the interactive session (session 1).** The scheduled task
  does; an ssh-spawned `windows-mcp serve` lands in **session 0** where there is
  no desktop: `Snapshot` returns "No windows found" and nothing can be clicked.
  Verify first:
  ```js
  const p = await tools.windows.PowerShell({
    command: '$p=[System.Diagnostics.Process]::GetCurrentProcess(); "session=$($p.SessionId) interactive=$([Environment]::UserInteractive)"; query session'
  });
  // want: session=1 interactive=True, and a console session with felix Active
  ```
- VM: Windows 11, user `felix`, display 1280x1024 (coordinates in screenshots are
  1:1 screen pixels). Excel (Office16), SSAS 2025, SSMS 22, Tabular Editor.
- The VM's Excel here is **visible and separate** from the excel-mcp's hidden
  Excel instances (those run in session 0 over ssh). The test workbook
  `C:\Users\Public\Documents\manual-mallardcube-proxy-test.xlsm` is usually open
  in session 1's Excel; the excel-mcp sessions cannot open it (file lock).

## Calling convention (Code Mode)

```js
await tools.windows.Snapshot({ use_vision: false, use_ui_tree: true, use_annotation: true });
await tools.windows.Click({ loc: [x, y] });            // absolute screen pixels
await tools.windows.Shortcut({ shortcut: "shift+f10" }); // context menu on selection
await tools.windows.Screenshot({ use_annotation: true }); // image + window summary
await tools.windows.Wait({ duration: 2 });               // INTEGER seconds only
```

Tools: `Snapshot`, `Screenshot`, `Click`, `Type`, `Scroll`, `Move`, `Shortcut`,
`Wait`, `WaitFor`, `App` (launch/switch/resize), `PowerShell`, `FileSystem`,
`Clipboard`, `Process`, `Registry`, `DisplayInventory`.

## Core workflow

1. `Screenshot` to see the real state (read the image; it is annotated with the
   cursor position and window list).
2. `Snapshot` when you need element coordinates/labels (it prints a UIA tree
   with `(x,y) label [action: click]` entries).
3. Click/Shortcut, then **screenshot again to verify** — never assume.
4. For anything Excel can do through COM, prefer COM (see below); use clicks
   only for UI-only surfaces (context menus, dialogs, field lists).

## Excel via COM from the PowerShell tool (the safe path)

```powershell
$xl = [Runtime.InteropServices.Marshal]::GetActiveObject('Excel.Application')
$ws = $xl.ActiveWorkbook.Worksheets.Item('Sheet1')
$ws.Cells.Item(6,1).Select(); $xl.ActiveWindow.Activate()   # then Shift+F10 opens the cell menu
$pt = $ws.PivotTables(1); $pt.RefreshTable()                # data refresh (no REFRESH CUBE; see proxy-excel-test)
$pt.RowRange.Cells | ForEach-Object { $_.Text }             # read pivot row labels
$xl.ActiveWorkbook.SaveCopyAs("C:\Users\Public\Documents\probe.xlsm")
```

Useful facts learned here:

- **Add an OLAP field to a pivot with `CubeField.Orientation`**, not `AddFields`
  (refused for OLAP: "Unable to get the AddFields property"):
  `$pt.CubeFields('[DateDim].[FullDate]').Orientation = 1` (1=row, 2=column,
  4=data). Measures: `$pt.CubeFields('[Measures].[Revenue]').Orientation = 4`.
- Create an OLAP pivot programmatically (works when `CommandType = 1`, i.e.
  `xlCmdCube` — with `3` Excel says "can't find table"):
  ```powershell
  $conn = $wb.Connections.Add2('Ref', 'ref',
    'OLEDB;Provider=MSOLAP.8;Integrated Security=SSPI;Persist Security Info=True;Initial Catalog=<db>;Data Source=localhost;MDX Compatibility=1;Safety Options=2;MDX Missing Member Mode=Error;Update Isolation Level=2',
    '<cube>', 1, $false, $false)
  $pc = $wb.PivotCaches().Create(2, $conn)          # 2 = xlExternal
  $pt = $pc.CreatePivotTable($ws.Range('A3'), 'RefPivot')
  ```
- `SaveCopyAs` follows Excel's **default save format** (this VM saved an ODF
  file with a `.xlsx` name). For an xlsx you can unzip, use
  `$wb.SaveAs(path, 51)` (xlOpenXMLWorkbook).
- Menu geometry: after `Shift+F10`, **take a screenshot and read the item's
  position before clicking** (see gotchas).

## Gotchas (learned the hard way)

- **`Wait` takes an integer** — `{duration: 1.5}` errors and aborts the whole
  `execute` block (already-run calls stay applied).
- **Context menus open relative to the cursor**, so their geometry differs from
  the last time. Always screenshot the open menu and click the exact row. One
  stray click hit **Delete PivotTable**; `Ctrl+Z` restored it (workbook was
  unsaved). Prefer the keyboard-free path: select the cell via COM, Shift+F10,
  screenshot, click.
- **Modal dialogs block everything** (a hung Excel once had to be killed). If an
  operation shows a dialog, find its button in a screenshot and click it before
  doing anything else.
- **Snapshot truncates at 500 elements** ("reached the 500-element capture
  limit"). For menus/dialogs use a `Screenshot` instead, or narrow the view.
- `Esc` closes menus; `Ctrl+Z` undoes (including a deleted pivot table).
- Excel's PivotTable Fields pane can be toggled via
  `$xl.ShowPivotTableFieldList = $true` (or right-click → Show Field List).
- The UI tree does not show every Excel surface (pivot field dropdowns, ribbon
  popups are unreliable) — screenshots + coordinates are the fallback.
- Keep `DisplayInventory`/`Screenshot` in mind on a disconnected RDP session:
  UIA still works but screenshots can go black; ask the user to keep the session
  connected for pixel work.

## SSAS HTTP tracing: the pump + a plain-XML relay (no Fiddler needed)

The VM has the real **`msmdpump.dll`** ISAPI pump deployed, so Excel can talk to
SSAS 2025 over HTTP — and with one header stripped the pump answers **plain
`text/xml`** instead of the binary `application/sx+xpress`, which makes tracing
trivial.

**Pump layout** (IIS site `Default Web Site`, app `olap`, app pool
`SSAS_AppPool`):

- `C:\inetpub\wwwroot\olap\msmdpump.dll` (+ `Resources\1033\*.rll`), config in
  `msmdpump.ini` next to it: `<ServerName>localhost:2383</ServerName>` targets
  the default SSAS instance (the `MallardRef` reference). The app's handler
  `SSAS ISAPI Handler` maps `*.dll` to the pump (the built-in `ISAPI-dll` is
  disabled by default and must be enabled too).
- **The pump refuses plain HTTP**: a POST to `http://…/olap/msmdpump.dll`
  returns `500` with the SOAP fault *"Connections to SQL Server Analysis
  Services through msmdpump.dll must use secure channels, for example
  HTTPS."* — an HTTPS binding is required (we use port **8443**).
- The ini and IIS config are ACL-protected: the interactive user is a
  UAC-*filtered* admin, so writes need an elevated process. On this VM
  `Start-Process powershell -Verb RunAs …` elevates **without a prompt**
  (`ConsentPromptBehaviorAdmin=0`); registering a scheduled task with
  `-RunLevel Highest` fails with "Access is denied". HTTPS setup (elevated):
  `New-SelfSignedCertificate -DnsName localhost,win11 -CertStoreLocation
  Cert:\LocalMachine\My`, export+import to `Cert:\LocalMachine\Root`,
  `New-WebBinding -Name 'Default Web Site' -Protocol https -Port 8443`,
  `netsh http add sslcert ipport=0.0.0.0:8443 certhash=<thumb>
  appid={4dc3e181-e14b-4a21-b022-59fc669b0914}`.

**The relay** (`C:\Users\Public\Documents\pump-proxy2.ps1`): a C# `HttpListener`
on `127.0.0.1:8090` forwarding to `https://localhost:8443`, which

- **strips `X-Transport-Caps-Negotiation-Flags`** (and `Accept-Encoding`) from
  the forwarded request. MSOLAP sends `0,1,0,1,1`; when the pump sees it it
  answers `application/sx+xpress`. Stripped, it answers `Content-Type:
  text/xml` (response caps `0,0,0,0,0`);
- logs bodies **and headers** to `C:\Users\Public\Documents\pumpproxy\` as
  `NNN_req.xml`, `NNN_resp.xml`, `NNN_req_headers.txt`, `NNN_resp_headers.txt`
  (restart the relay to reset the counter).

Point Excel at `Data Source=http://127.0.0.1:8090/OLAP/msmdpump.dll`,
`Initial Catalog=MallardRef`, cube `Model`. MSOLAP adds `X-AS-ActivityID`,
`X-AS-RequestID`, `X-AS-CurrentActivityID`, `X-AS-SessionID`,
`SspropInitAppName: Excel` and a `SOAPAction` header; the pump never echoes the
activity IDs. MSOLAP **bypasses the system proxy for `localhost`/`127.0.0.1`** —
to get Fiddler to see it, use `ipv4.fiddler` as the host (Fiddler's loopback
alias); our own relay is a real destination, so it needs no such trick.

Native (TCP) capture, when the pump is not in play: `relay.ps1` forwards
`localhost:2399 → 2383` and logs the native protocol. There the payloads are
`sx+xpress` and decode with `ntdll!RtlDecompressBuffer` (format 3) starting a
few bytes into the payload; the output is UTF-16.

## Excel pivot expand testing (learned the hard way)

- **Gesture**: select the member cell via COM (`$pt.TableRange2.Cells.Item(r,1).Select()`),
  send `Shift+F10`, screenshot, click `Expand/Collapse`, then `Expand`.
  Coordinates move with the window/cell — always screenshot the open menu.
  `Drill Down/Drill Up` is the row below; a stray click there drills instead.
- The expand MDX is `Hierarchize(DrilldownMember({{DrilldownLevel({All})}},
  {member}))`. **What Excel asks for in `DIMENSION PROPERTIES` depends on the
  pivot cache's metadata**, which is built from `MDSCHEMA_PROPERTIES` at pivot
  creation:
  - advertising the standard member properties (MEMBER_CAPTION, MEMBER_NAME, …)
    makes Excel request ~38 properties and (with MallardCube before the fix)
    silently ignore the expansion response;
  - advertising the tabular reference shape (per level `KEY0` + `MEMBER_VALUE`,
    `NAME` on `(All)`, `PROPERTY_TYPE=5`, no cell properties) keeps it on the
    short list (`PARENT_UNIQUE_NAME,[Year]KEY0,[Year]MEMBER_VALUE,…`) plus
    `WHERE ([Measures].[Revenue])`, and the expansion renders.
- **After changing metadata, build a fresh pivot** (`Connections.Add2` →
  `PivotCaches().Create(2, $conn)` → `CreatePivotTable`) — `RefreshTable()`
  reuses the old cache field metadata.
- **Multi-instance Excel**: `GetActiveObject('Excel.Application')` returns one
  registered instance, not necessarily yours. A new instance created with
  `New-Object -ComObject Excel.Application` cannot be reattached from a later
  tool call (each PowerShell call is a fresh process) — symptoms are
  `Workbooks.Count = 0` and "You cannot call a method on a null-valued
  expression". Drive the existing instance instead.
- **Modal dialogs block COM** (same null-valued symptoms). Screenshot and click
  the dialog's button before anything else; a failed connection swap can leave
  the "Analysis Services Connection" wizard open, and a bad catalog shows
  "Errors in the OLE DB provider … the catalog does not exist".
- `SaveAs(path, 51)` for an unzip-able xlsx; `SaveCopyAs` follows the default
  format (it wrote an ODF file with an .xlsx name here).

## Excel date-filter dialog (corrected 2026-09-22)

- The **Filter → Date Filters...** item opens a `Date Filter (<field>)` dialog
  (condition combo, date field, calendar button, `Whole Days`, OK/Cancel).
  Office draws its buttons itself — there are **no child HWNDs**, `WM_COMMAND`
  and UIA `Invoke` do nothing, and `WindowFromPoint` reports Excel's main
  window (XLMAIN) even over the dialog.
- **Clicks do reach it** once the dialog is the foreground window (the same
  injection that drives the context menus). The trap is the date field: typed
  text is *not committed* until the **calendar picker** sets a value, and OK
  stays **disabled** until then — so an OK click silently does nothing.
  Symptom: `Cancel` closes the dialog, `OK` does not.
- Recipe that works: click the calendar button → click a day (or `Today`) in
  the picker → the field shows the picked date → click `OK`.
- Keyboard: typing reaches the focused RichEdit and `Shift+Tab` moves focus
  (the combo responds to `Down`), but `Tab` from the RichEdit is swallowed and
  `Enter`/`Space` do not press the buttons — use the mouse for OK/Cancel.
- The dialog blocks COM: querying a pivot while it is open returns nulls.

## Safety

The endpoint is **unauthenticated on the LAN** and has full system access
(PowerShell, Registry, FileSystem). Recommend `--auth-key` on the server plus an
`Authorization` header in `mcp.windows` before leaving it running long-term.
