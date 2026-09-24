param([string]$Source, [string]$Tag, [string]$OutFile)
$owned = $false
try { $xl = [Runtime.InteropServices.Marshal]::GetActiveObject('Excel.Application') }
catch {
  $xl = New-Object -ComObject Excel.Application
  $xl.Visible = $false
  $owned = $true
}
$xl.DisplayAlerts = $false
$url = if ($Source -eq 'proxy') { 'http://ssasproxy:8080/xmla?v=' + $Tag } else { 'http://127.0.0.1:8090/OLAP/msmdpump.dll?v=' + $Tag }
$catalog = if ($Source -eq 'proxy') { 'SALES_ANALYTICS' } else { 'MallardDemo' }
$cube = if ($Source -eq 'proxy') { 'Sales' } else { 'Model' }
$connStr = 'OLEDB;Provider=MSOLAP.8;Integrated Security=SSPI;Persist Security Info=True;Initial Catalog=' + $catalog + ';Data Source=' + $url + ';MDX Compatibility=1;Safety Options=2;MDX Missing Member Mode=Error;Update Isolation Level=2'

$specs = @(
  @{ n='rows2';              rows=@('[Category].[Category]','[Channel].[Channel]'); cols=@(); data=@('[Measures].[Revenue]') },
  @{ n='rows2_terr';         rows=@('[Category].[Category]','[Territory].[Territory]'); cols=@(); data=@('[Measures].[Revenue]') },
  @{ n='cols1';              rows=@('[Category].[Category]'); cols=@('[Date].[Calendar]'); data=@('[Measures].[Revenue]') },
  @{ n='cols2';              rows=@('[Category].[Category]'); cols=@('[Date].[Calendar]','[Channel].[Channel]'); data=@('[Measures].[Revenue]') },
  @{ n='data2';              rows=@('[Category].[Category]'); cols=@(); data=@('[Measures].[Revenue]','[Measures].[Units]') },
  @{ n='rows2_cols1';        rows=@('[Category].[Category]','[Channel].[Channel]'); cols=@('[Date].[Calendar]'); data=@('[Measures].[Revenue]') },
  @{ n='rows2_cols1_data2';  rows=@('[Category].[Category]','[Channel].[Channel]'); cols=@('[Date].[Calendar]'); data=@('[Measures].[Revenue]','[Measures].[Units]') },
  @{ n='page_terr';          rows=@('[Category].[Category]'); cols=@(); data=@('[Measures].[Revenue]'); page='[Territory].[Territory]' },
  @{ n='totals_off';         rows=@('[Category].[Category]','[Channel].[Channel]'); cols=@('[Date].[Calendar]'); data=@('[Measures].[Revenue]'); noTotals=$true },
  @{ n='sort_value';         rows=@('[Category].[Category]'); cols=@(); data=@('[Measures].[Revenue]'); sort='[Category].[Category]' },
  @{ n='top5';               rows=@('[Category].[Category]'); cols=@(); data=@('[Measures].[Revenue]'); top=5 },
  @{ n='meas_on_rows';       rows=@(); cols=@('[Category].[Category]'); data=@('[Measures].[Revenue]','[Measures].[Units]'); measRows=$true },
  @{ n='crosstab';           rows=@('[Category].[Category]'); cols=@('[Channel].[Channel]'); data=@('[Measures].[Revenue]') }
)

$out = @()
foreach ($s in $specs) {
  $log = "### " + $s.n
  $wb = $null
  try {
    $wb = $xl.Workbooks.Add()
    $conn = $wb.Connections.Add2('S', 's', $connStr, $cube, 1, $false, $false)
    $conn.OLEDBConnection.Refresh() | Out-Null
    $pc = $wb.PivotCaches().Create(2, $conn)
    $pt = $pc.CreatePivotTable($wb.Worksheets.Item(1).Range('A3'), 'SP')
    $script:PT = $pt
    foreach ($r in $s.rows) { $f = $null; foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $r) { $f = $cf } }; if ($f) { $f.Orientation = 1 } else { throw "no field $r" } }
    foreach ($c in $s.cols) { $f = $null; foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $c) { $f = $cf } }; if ($f) { $f.Orientation = 2 } else { throw "no field $c" } }
    foreach ($d in $s.data) { $f = $null; foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $d) { $f = $cf } }; if ($f) { $f.Orientation = 4 } else { throw "no field $d" } }
    if ($s.measRows) { foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq '[Measures].[Revenue]') { $cf.Orientation = 1 } } }
    if ($s.page) {
      foreach ($cf in $script:PT.CubeFields()) { if ($cf.Name -eq $s.page) { $cf.Orientation = 3 } }
      $pt.RefreshTable() | Out-Null
      foreach ($p in $pt.PivotFields()) {
        if ($p.Name -like ($s.page + '*')) {
          $items = @(); foreach ($it in $p.PivotItems()) { $items += $it.Name }
          $pick = @($items | Where-Object { $_ -notmatch 'All' } | Select-Object -First 1)
          if ($pick.Count -eq 1) { $p.CurrentPage = $pick[0]; $log += " [page=" + $pick[0] + "]" }
        }
      }
    }
    if ($s.sort) { foreach ($p in $pt.PivotFields()) { if ($p.Name -like ($s.sort + '*')) { try { $p.AutoSort(2, '[Measures].[Revenue]') } catch { $log += " [sort ERR]" } } } }
    if ($s.top) { foreach ($p in $pt.PivotFields()) { if ($p.Name -like '*Category*') { try { $p.PivotFilters.Add2(6, '[Measures].[Revenue]', $s.top) } catch { $log += " [top ERR]" } } } }
    if ($s.noTotals) {
      try { $pt.RowGrand = $false } catch { $log += " [RowGrand ERR]" }
      try { $pt.ColumnGrand = $false } catch { $log += " [ColumnGrand ERR]" }
      foreach ($p in $pt.PivotFields()) { try { if ($p.Orientation -eq 1) { $p.Subtotals = @() } } catch {} }
    }
    $pt.RefreshTable() | Out-Null
    $rng = $pt.TableRange2
    $rows = $rng.Rows.Count; $cols = $rng.Columns.Count
    $log += " (grid " + $rows + "x" + $cols + ")"
    for ($r = 1; $r -le [Math]::Min($rows, 14); $r++) {
      $line = @()
      for ($c = 1; $c -le [Math]::Min($cols, 9); $c++) { $line += (([string]$rng.Cells.Item($r,$c).Text) -replace '\s+',' ').Trim() }
      $log += "| " + ($line -join ' | ')
    }
  } catch { $log += " ERROR: " + $_.Exception.Message.Substring(0, [Math]::Min(90, $_.Exception.Message.Length)) }
  if ($wb) { try { $wb.Close($false) } catch {} }
  $out += $log
}
$text = $out -join [Environment]::NewLine
$text | Set-Content -Path $OutFile -Encoding UTF8
if ($owned) { try { $xl.Quit() } catch {} }
"wrote " + $OutFile + " (" + $text.Length + " chars) owned=" + $owned
