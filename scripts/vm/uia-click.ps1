# uia-click.ps1 — click a UI Automation element by name or automation id.
#
# The reusable half of the VM's UI scripting: Office dialogs draw their own
# controls (no child HWNDs) and ignore WM_COMMAND, so the click is a real
# synthetic mouse click at the element's clickable point, with the window
# foregrounded first (Office dialogs ignore clicks when inactive).
#
# Use uia-probe.ps1 to find the element. -DryRun prints the target and clicks
# nothing — always dry-run a new sequence first.
#
# Usage:
#   powershell ... -File uia-click.ps1 -Title 'Date Filter' -Name 'Today' -DryRun
#   powershell ... -File uia-click.ps1 -Title 'Date Filter' -Name 'OK'
#   powershell ... -File uia-click.ps1 -Title 'X' -AutomationId '1234'

param(
    [Parameter(Mandatory = $true)][string]$Title,
    [string]$Name = '',
    [string]$AutomationId = '',
    [switch]$DryRun,
    [int]$TimeoutSeconds = 10
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Mouse {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    public const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004;
    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        System.Threading.Thread.Sleep(120);
        mouse_event(LEFTDOWN, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(60);
        mouse_event(LEFTUP, 0, 0, 0, UIntPtr.Zero);
    }
}
'@

if (-not $Name -and -not $AutomationId) {
    Write-Error 'provide -Name or -AutomationId'
    exit 2
}

$root = [System.Windows.Automation.AutomationElement]::RootElement
$windowCondition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::NameProperty, $Title)

$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$window = $null
$target = $null
while ((Get-Date) -lt $deadline -and -not $target) {
    $window = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $windowCondition)
    if ($window) {
        if ($Name) {
            $elementCondition = New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::NameProperty, $Name)
        } else {
            $elementCondition = New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $AutomationId)
        }
        $target = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $elementCondition)
    }
    if (-not $target) { Start-Sleep -Milliseconds 500 }
}

if (-not $target) {
    Write-Error "element not found (title '$Title', name '$Name', id '$AutomationId')"
    exit 1
}

$null = [Mouse]::SetForegroundWindow([IntPtr]$window.Current.NativeWindowHandle)
Start-Sleep -Milliseconds 300

try {
    $point = $target.GetClickablePoint()
    $x = [int]$point.X
    $y = [int]$point.Y
} catch {
    $rect = $target.Current.BoundingRectangle
    $x = [int]($rect.X + $rect.Width / 2)
    $y = [int]($rect.Y + $rect.Height / 2)
}

if ($DryRun) {
    Write-Output ("DRY RUN: would click '{0}' at ({1},{2})" -f $target.Current.Name, $x, $y)
    exit 0
}

[Mouse]::Click($x, $y)
Write-Output ("clicked '{0}' at ({1},{2})" -f $target.Current.Name, $x, $y)
