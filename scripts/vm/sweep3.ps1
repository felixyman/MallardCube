param([string]$Source, [string]$Tag, [string]$OutFile)
$xl = New-Object -ComObject Excel.Application
$xl.Visible = $false
$xl.DisplayAlerts = $false
$url = if ($Source -eq 'proxy') { 'http://ssasproxy:8080/xmla?v=' + $Tag } else { 'http://127.0.0.1:8090/OLAP/msmdpump.dll?v=' + $Tag }
$catalog = if ($Source -eq 'proxy') { 'SALES_ANALYTICS' } else { 'MallardDemo' }
$cube = if ($Source -eq 'proxy') { 'Sales' } else { 'Model' }
$connStr = 'OLEDB;Provider=MSOLAP.8;Integrated Security=SSPI;Persist Security Info=True;Initial Catalog=' + $catalog + ';Data Source=' + $url + ';MDX Compatibility=1;Safety Options=2;MDX Missing Member Mode=Error;Update Isolation Level=2'

$specs = @(
  @{ n='page_terr';            rows=@('[Category].[Category]'); data=@('[Measures].[Revenue]'); page='[Territory].[Territory]' },
  @{ n='top5';                 rows=@('[Category].[Category]'); data=@('[Measures].[Revenue]'); top=5 },
  @{ n='nested_value_filter';  rows=@('[Category].[Category]','[Channel].[Channel]'); data=@('[Measures].[Revenue]'); valueFilter=26000000 },
  @{ n='nested_label_filter';  rows=@('[Category].[Category]','[Channel].[Channel]'); data=@('[Measures].[Revenue]'); labelFilter='B' },
  @{ n='page_multiselect';     rows=@('[Channel].[Channel]'); data=@('[Measures].[Revenue]'); page='[Category].[Category]'; pageMembers=2 },
  @{ n='two_hier_same_dim';    rows=@('[Date].[Calendar]'); data=@('[Measures].[Revenue]'); page='[Date].[Full Date]' },
  @{ n='values_as_pct';        rows=@('[Category].[Category]'); data=@('[Measures].[Revenue]'); showAs='pct' },
  @{ n='number_format';        rows=@('[Category].[Category]'); data=@('[Measures].[Revenue]'); numFmt='#,##0' },
  @{ n='drillthrough_ct';      rows=@('[Category].[Category]'); cols=@('[Channel].[Channel]'); data=@('[Measures].[Revenue]'); drillthrough=$true },
  @{ n='two_pivots_one_cache'; rows=@('[Category].[Category]'); data=@('[Measures].[Revenue]'); secondPivot=$true },
  @{ n='subtotals_off_first';  rows=@('[Category].[Category]','[Channel].[Channel]'); data=@('[Measures].[Revenue]'); subOff=$true }
)

$out = @()
foreach ($s in $specs) {
  $log = "### " + $s.n
  $wb = $null
  try {
    $wb = $xl.Workbooks.Add()
    $conn = $wb.Connections.Add2('S', 's', $connStr, $cube, 1, $false, $false)
    $conn.OLEDBConnection.Refresh() | Out-Null
    # A cold connection keeps Excel busy for seconds; automation calls made
    # meanwhile are rejected (RPC_E_CALL_REJECTED), so let it settle.
    Start-Sleep -Seconds 3
    $pc = $wb.PivotCaches().Create(2, $conn)
    $pt = $pc.CreatePivotTable($wb.Worksheets.Item(1).Range('A3'), 'SP')
    $script:PT = $pt
    foreach ($r in $s.rows) { $f = $null; foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $r) { $f = $cf } }; if ($f) { $f.Orientation = 1 } else { throw "no field $r" } }
    foreach ($c in $s.cols) { $f = $null; foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $c) { $f = $cf } }; if ($f) { $f.Orientation = 2 } else { throw "no field $c" } }
    foreach ($d in $s.data) { $f = $null; foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $d) { $f = $cf } }; if ($f) { $f.Orientation = 4 } else { throw "no field $d" } }
    if ($s.secondPivot) {
      $pt2 = $pc.CreatePivotTable($wb.Worksheets.Add().Range('A3'), 'SP2')
      foreach ($cf in $pt2.CubeFields()) { if ($cf.Name -eq '[Category].[Category]') { $cf.Orientation = 1 } }
      foreach ($cf in $pt2.CubeFields()) { if ($cf.Name -eq '[Measures].[Revenue]') { $cf.Orientation = 4 } }
      $pt2.RefreshTable() | Out-Null
      $log += " [second pivot " + $pt2.TableRange2.Rows.Count + "x" + $pt2.TableRange2.Columns.Count + "]"
    }
    if ($s.page) {
      foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $s.page) { $cf.Orientation = 3 } }
      # Excel's OLAP filter refresh is asynchronous: reading the range too early
    # shows the pre-filter grid (recorded 2026-09-26).
    try { $conn.OLEDBConnection.BackgroundQuery = $false } catch {}
    $pt.RefreshTable() | Out-Null
      foreach ($p in $pt.PivotFields()) {
        if ($p.Orientation -eq 3) {
          $items = @(); foreach ($it in $p.PivotItems()) { $items += $it.Name }
          if ($items.Count -eq 0) {
            # Items populate only once the field is laid out: park it on rows, read
            # the members, then move it back to the page area.
            $p.Orientation = 1
            $pt.RefreshTable() | Out-Null
            $items = @(); foreach ($it in $p.PivotItems()) { $items += $it.Name }
            $p.Orientation = 3
            $pt.RefreshTable() | Out-Null
          }
          $pick = @($items | Where-Object { $_ -notmatch 'All' } | Select-Object -First $(if ($s.pageMembers) { $s.pageMembers } else { 1 }))
          if ($pick.Count -eq 1) { $p.CurrentPage = $pick[0]; $log += " [page=" + $p.CurrentPage + "]" }
          elseif ($pick.Count -gt 1) { try { $p.VisibleItemsList = $pick; $log += " [multi=" + ($pick -join ',') + "]" } catch { $log += " [multi ERR]" } }
        }
      }
    }
    if ($s.top) {
      $cat = $null; foreach ($p in $pt.PivotFields()) { if ($p.Name -like '*Category*') { $cat = $p } }
      try { $cat.PivotFilters.Add2(6, $pt.PivotFields('[Measures].[Revenue]'), $s.top); $log += " [top=" + $cat.PivotFilters.Count + "]" } catch { $log += " [top ERR]" }
    }
    if ($s.valueFilter) {
      $cat = $null; foreach ($p in $pt.PivotFields()) { if ($p.Name -like '*Category*') { $cat = $p } }
      try { $cat.PivotFilters.Add2(10, $pt.PivotFields('[Measures].[Revenue]'), $s.valueFilter); $log += " [valueFilter=" + $cat.PivotFilters.Count + "]" } catch { $log += " [valueFilter ERR]" }
    }
    if ($s.labelFilter) {
      $cat = $null; foreach ($p in $pt.PivotFields()) { if ($p.Name -like '*Category*') { $cat = $p } }
      try { $cat.PivotFilters.Add2(18, [Type]::Missing, $s.labelFilter); $log += " [labelFilter=" + $cat.PivotFilters.Count + "]" } catch { $log += " [labelFilter ERR]" }
    }
    if ($s.showAs) {
      try { $pt.DataFields(1).Calculation = 8; $log += " [pct]" } catch { $log += " [pct ERR]" }
    }
    if ($s.numFmt) {
      try { $pt.DataFields(1).NumberFormat = $s.numFmt; $log += " [fmt]" } catch { $log += " [fmt ERR]" }
    }
    if ($s.subOff) {
      $first = $null; foreach ($p in $pt.PivotFields()) { if ($p.Orientation -eq 1 -and $null -eq $first) { $first = $p } }
      try { $first.Subtotals = @(); $log += " [subOff]" } catch { $log += " [subOff ERR]" }
    }
    $pt.RefreshTable() | Out-Null
    if ($s.drillthrough) {
      try {
        $pt.TableRange2.Cells.Item(2,2).ShowDetail = $true
        $ws = $wb.Worksheets.Item($wb.Worksheets.Count)
        $log += " [drillthrough rows=" + $ws.UsedRange.Rows.Count + "]"
      } catch { $log += " [drillthrough ERR]" }
    }
    $rng = $pt.TableRange2
    $rows = $rng.Rows.Count; $cols = $rng.Columns.Count
    $log += " (grid " + $rows + "x" + $cols + ")"
    for ($r = 1; $r -le [Math]::Min($rows, 12); $r++) {
      $line = @()
      for ($c = 1; $c -le [Math]::Min($cols, 7); $c++) { $line += (([string]$rng.Cells.Item($r,$c).Text) -replace '\s+',' ').Trim() }
      $log += "| " + ($line -join ' | ')
    }
  } catch { $log += " ERROR: " + $_.Exception.Message.Substring(0, [Math]::Min(90, $_.Exception.Message.Length)) }
  if ($wb) { try { $wb.Close($false) } catch {} }
  $out += $log
}
$text = $out -join [Environment]::NewLine
$text | Set-Content -Path $OutFile -Encoding UTF8
try { $xl.Quit() } catch {}
"wrote " + $OutFile + " (" + $text.Length + " chars)"

