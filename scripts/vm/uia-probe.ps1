# uia-probe.ps1 — dump a window's UI Automation tree instead of screenshotting.
#
# Use this when a dialog blocks a run or a scripted sequence needs element
# names/ids: one command shows every control with its name, automation id,
# class, bounding rectangle and enabled/offscreen flags.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File uia-probe.ps1 -ListWindows
#   powershell ... -File uia-probe.ps1                      # focused window
#   powershell ... -File uia-probe.ps1 -Title 'Date Filter'
#   powershell ... -File uia-probe.ps1 -Title 'Date Filter' -MaxDepth 3
#
# Names/ids printed here are what uia-click.ps1 accepts.

param(
    [string]$Title = '',
    [switch]$ListWindows,
    [int]$MaxDepth = 6
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$root = [System.Windows.Automation.AutomationElement]::RootElement

if ($ListWindows) {
    $windows = $root.FindAll(
        [System.Windows.Automation.TreeScope]::Children,
        [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($window in $windows) {
        if ($window.Current.Name) {
            Write-Output ("{0}  [{1}]" -f $window.Current.Name, $window.Current.ClassName)
        }
    }
    exit 0
}

if ($Title) {
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Title)
    $target = $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $condition)
} else {
    $target = [System.Windows.Automation.AutomationElement]::FocusedElement
}

if (-not $target) {
    Write-Error "no window found for title '$Title'"
    exit 1
}

Write-Output ("window: {0} [{1}]" -f $target.Current.Name, $target.Current.ClassName)

function Walk($element, [int]$depth) {
    if ($depth -gt $MaxDepth) { return }
    $rect = $element.Current.BoundingRectangle
    Write-Output ("{0}{1} '{2}' id='{3}' class='{4}' rect={5},{6} {7}x{8} enabled={9} offscreen={10}" -f `
        ('  ' * $depth),
        $element.Current.ControlType.ProgrammaticName.Replace('ControlType.', ''),
        $element.Current.Name,
        $element.Current.AutomationId,
        $element.Current.ClassName,
        [int]$rect.X, [int]$rect.Y, [int]$rect.Width, [int]$rect.Height,
        $element.Current.IsEnabled,
        $element.Current.IsOffscreen)
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $child = $walker.GetFirstChild($element)
    while ($child) {
        Walk $child ($depth + 1)
        $child = $walker.GetNextSibling($child)
    }
}

Walk $target 0
