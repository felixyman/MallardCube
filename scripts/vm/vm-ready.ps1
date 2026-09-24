# vm-ready.ps1 — bring the VM to a known-good state for proxy/mirror work.
#
# Run this first in any VM session; it is idempotent. It:
#   * starts the plain-HTTP listener on 8090 in front of the HTTPS pump
#     (pump-proxy2.ps1), which does not survive a reboot;
#   * clears the WinINET proxy — a stale 127.0.0.1:8888 entry with
#     <-loopback> makes MSOLAP and Excel fail while curl keeps working;
#   * kills stale Excel automation instances;
#   * verifies SSAS, IIS, the mirror chain, ADOMD, and an optional proxy URL;
#   * prints a readiness table and exits non-zero when something is missing.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File vm-ready.ps1
#   powershell ... -File vm-ready.ps1 -ProxyUrl http://127.0.0.1:8099/status

param([string]$ProxyUrl = '')

$script:ready = $true

function Check([string]$Name, [scriptblock]$Probe) {
    try {
        $detail = & $Probe
        if ($detail) {
            Write-Output ("  OK    {0,-26} {1}" -f $Name, $detail)
        } else {
            $script:ready = $false
            Write-Output ("  MISS  {0,-26}" -f $Name)
        }
    } catch {
        $script:ready = $false
        Write-Output ("  MISS  {0,-26} {1}" -f $Name, $_.Exception.Message)
    }
}

Write-Output '== services =='
Check 'SSAS (OLAP service)' {
    if ((Get-Service MSSQLServerOLAPService -ErrorAction SilentlyContinue).Status -eq 'Running') { 'running' }
}
Check 'IIS (W3SVC)' {
    if ((Get-Service W3SVC -ErrorAction SilentlyContinue).Status -eq 'Running') { 'running' }
}

Write-Output '== WinINET proxy (clear BEFORE any loopback HTTP check) =='
$settings = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
if ($settings.ProxyEnable -eq 1) {
    Write-Output ("  clearing ProxyEnable (was {0})" -f $settings.ProxyServer)
    Set-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -Name ProxyEnable -Value 0
}
Check 'no WinINET proxy' {
    if ((Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings').ProxyEnable -eq 0) { 'direct' }
}

Write-Output '== mirror chain (Excel needs 8090: plain HTTP -> https://localhost:8443) =='
if (-not (netstat -ano | Select-String ':8090\s+.*LISTENING')) {
    Write-Output '  starting pump-proxy2.ps1'
    Start-Process powershell -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass',
        '-File', 'C:\Users\Public\Documents\pump-proxy2.ps1' -WindowStyle Hidden
    Start-Sleep -Seconds 3
}
Check '8090 listener' { if (netstat -ano | Select-String ':8090\s+.*LISTENING') { 'listening' } }
Check 'mirror Execute' {
    $body = '<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body>' +
        '<Execute xmlns="urn:schemas-microsoft-com:xml-analysis"><Command>' +
        '<Statement>SELECT {[Measures].[Revenue]} ON COLUMNS FROM [Model]</Statement></Command>' +
        '<Properties><PropertyList><Catalog>MallardDemo</Catalog></PropertyList></Properties>' +
        '</Execute></soap:Body></soap:Envelope>'
    $response = Invoke-WebRequest -Uri 'http://127.0.0.1:8090/OLAP/msmdpump.dll' -Method POST `
        -Body $body -ContentType 'text/xml' -UseBasicParsing -TimeoutSec 20
    if ($response.Content -match 'ExecuteResponse' -and $response.Content -match '<Value[^>]*>([^<]*)</Value>') {
        'revenue=' + $Matches[1]
    }
}

Write-Output '== reference data (ADOMD, native) =='
Check 'MallardDemo native query' {
    Add-Type -Path 'C:\Program Files\Microsoft.NET\ADOMD.NET\170\Microsoft.AnalysisServices.AdomdClient.dll'
    $connection = New-Object Microsoft.AnalysisServices.AdomdClient.AdomdConnection(
        'Data Source=localhost;Initial Catalog=MallardDemo')
    $connection.Open()
    $command = $connection.CreateCommand()
    $command.CommandText = 'SELECT [Measures].[Revenue] ON 0 FROM [Model]'
    $reader = $command.ExecuteReader(); $null = $reader.Read(); $value = $reader.GetValue(0); $reader.Close()
    $connection.Close()
    "revenue=$value"
}

Write-Output '== automation state =='
$excel = @(Get-Process EXCEL -ErrorAction SilentlyContinue)
if ($excel.Count -gt 0) {
    Write-Output ("  killing {0} stale Excel instance(s)" -f $excel.Count)
    $excel | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
}
Check 'no stale Excel' { if (-not (Get-Process EXCEL -ErrorAction SilentlyContinue)) { 'clean' } }

if ($ProxyUrl) {
    Write-Output '== proxy =='
    Check $ProxyUrl {
        $response = Invoke-WebRequest -Uri $ProxyUrl -UseBasicParsing -TimeoutSec 5
        if ($response.StatusCode -eq 200) { 'HTTP 200' }
    }
}

Write-Output ''
if ($script:ready) {
    Write-Output 'VM READY'
} else {
    Write-Output 'VM NOT READY'
    exit 1
}
