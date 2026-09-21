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

## Safety

The endpoint is **unauthenticated on the LAN** and has full system access
(PowerShell, Registry, FileSystem). Recommend `--auth-key` on the server plus an
`Authorization` header in `mcp.windows` before leaving it running long-term.
