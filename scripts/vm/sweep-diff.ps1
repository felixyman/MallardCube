# sweep-diff.ps1 — run the sweep for one source and diff against its baseline.
#
# The baselines were captured against the mirror (MallardDemo / Model) and the
# proxy (SALES_ANALYTICS / Sales) and are versioned in the repo as
# parity/sweep*-baseline.txt, copied beside this script on the VM. One line is
# expected to move between runs: the two_hier_same_dim spec races a COM error
# message (0x800A03EC vs "Unable to set the Orientation property"); it fails
# against both engines either way, so it is filtered and reported as known.
#
# Usage:
#   powershell ... -File sweep-diff.ps1 -Source proxy
#   powershell ... -File sweep-diff.ps1 -Source mirror
#   powershell ... -File sweep-diff.ps1 -Source proxy -Sweep sweep2.ps1

param(
    [Parameter(Mandatory = $true)][ValidateSet('proxy', 'mirror')][string]$Source,
    [string]$Sweep = 'sweep3.ps1',
    [string]$Tag = 'diff'
)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$baseline = Join-Path $here ('sweep3-' + $Source + '-baseline.txt')
if ($Sweep -ne 'sweep3.ps1') {
    $baseline = Join-Path $here ($Sweep -replace '\.ps1$', '-' + $Source + '-baseline.txt')
}
if (-not (Test-Path $baseline)) {
    Write-Error "baseline not found: $baseline"
    exit 2
}

$outFile = Join-Path $here ('sweep-run-' + $Source + '-' + $Tag + '.txt')
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $here $Sweep) `
    -Source $Source -Tag $Tag -OutFile $outFile | Out-Null

$differences = Compare-Object (Get-Content $baseline) (Get-Content $outFile)
$real = @()
$known = 0
foreach ($difference in $differences) {
    if ($difference.InputObject -match 'two_hier_same_dim') {
        $known++
        continue
    }
    $real += $difference
}

if ($real.Count -eq 0) {
    Write-Output ("SWEEP OK: {0} matches {1} ({2} known flaky line(s))" -f `
        (Split-Path $outFile -Leaf), (Split-Path $baseline -Leaf), $known)
    exit 0
}

Write-Output ("SWEEP DIFF: {0} vs {1}" -f (Split-Path $outFile -Leaf), (Split-Path $baseline -Leaf))
$real | Format-Table -AutoSize | Out-String -Width 300
exit 1
