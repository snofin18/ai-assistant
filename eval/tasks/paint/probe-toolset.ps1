# TASK-309 Paint toolset probe.
#
# Purpose: bounded, read-only UIA dump of the Paint window so selector work can be
# calibrated from measured facts instead of guesses (see docs/memory/apps/paint.md 13).
#
# Boundaries:
#   - read-only: this script never clicks, never types, never moves the mouse.
#   - bounded: element count and depth are capped, and the walk stops on a wall-clock
#     deadline so a pathological tree cannot hang the shell (known pitfall: deep UIA
#     traversal can block while a modal flyout is open).
#   - ASCII only: refscan rejects non-ASCII bytes in .ps1.
#
# Usage:
#   powershell -NoProfile -File eval/tasks/paint/probe-toolset.ps1
#   powershell -NoProfile -File eval/tasks/paint/probe-toolset.ps1 -Filter Button

[CmdletBinding()]
param(
    [int]$MaxElements = 1200,
    [int]$MaxDepth = 30,
    [int]$DeadlineSeconds = 25,
    [string]$Filter = '',
    [switch]$IncludePatterns,
    [string]$ExpandAid = '',
    [string]$ThenExpandName = ''
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$UIA = [System.Windows.Automation.AutomationElement]
$Scope = [System.Windows.Automation.TreeScope]

function Get-PaintWindow {
    # Paint 11 is a packaged app; the process name is not stable across builds,
    # so match on process name OR window class instead of a single string.
    $root = $UIA::RootElement
    $kids = $root.FindAll($Scope::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($kid in $kids) {
        $ownerId = $kid.Current.ProcessId
        $proc = '?'
        try { $proc = (Get-Process -Id $ownerId -ErrorAction Stop).ProcessName } catch { }
        $cls = $kid.Current.ClassName
        if (($proc -match 'paint|mspaint') -or ($cls -match 'MSPaintApp|Paint')) {
            return $kid
        }
    }
    return $null
}

function Format-Element {
    param($Element)

    $cur = $Element.Current
    $name = $cur.Name
    $aid = $cur.AutomationId
    $type = $cur.ControlType.ProgrammaticName -replace '^ControlType\.', ''
    $focusable = $cur.IsKeyboardFocusable
    $offscreen = $cur.IsOffscreen
    # Virtualized / offscreen elements can report infinite bounds; Int32 casts would
    # throw and kill the whole walk, so collapse non-finite numbers to a sentinel.
    $r = $cur.BoundingRectangle
    $rect = '{0},{1},{2},{3}' -f (Format-Number $r.X), (Format-Number $r.Y), `
        (Format-Number $r.Width), (Format-Number $r.Height)

    # Supported patterns decide whether an element can be driven programmatically
    # (no mouse, no keyboard) or only by synthetic input. Worth the extra call here.
    $patterns = ''
    if ($IncludePatterns) {
        try {
            $names = @()
            foreach ($p in $Element.GetSupportedPatterns()) {
                $short = ($p.ProgrammaticName -split '\.')[0] -replace 'PatternIdentifiers$', ''
                $names += $short
            }
            if ($names.Count -gt 0) { $patterns = ' patterns=[' + ($names -join ',') + ']' }
            else { $patterns = ' patterns=[none]' }
        } catch {
            $patterns = ' patterns=[error]'
        }
    }

    # Keep the line one-per-element so the output can be diffed between runs.
    return ('name="{0}" aid="{1}" type={2} focusable={3} offscreen={4} rect={5}{6}' -f `
        $name, $aid, $type, $focusable, $offscreen, $rect, $patterns)
}

function Format-Number {
    param([double]$Value)
    if ([double]::IsNaN($Value) -or [double]::IsInfinity($Value)) { return 'inf' }
    return [string][int]$Value
}

$window = Get-PaintWindow
if ($null -eq $window) {
    Write-Output 'PROBE: paint-window-not-found'
    exit 1
}

$ownerId = $window.Current.ProcessId
$pname = '?'
try { $pname = (Get-Process -Id $ownerId -ErrorAction Stop).ProcessName } catch { }

Write-Output ('PROBE: window name="{0}" class="{1}" pid={2} proc={3}' -f `
    $window.Current.Name, $window.Current.ClassName, $ownerId, $pname)
Write-Output ('PROBE: caps max_elements={0} max_depth={1} deadline_s={2} filter="{3}"' -f `
    $MaxElements, $MaxDepth, $DeadlineSeconds, $Filter)

# Prefer UIA patterns over keyboard and synthetic input (rule 5: API first).
# ExpandCollapsePattern.Expand() opens a flyout in-process and moves nothing.
$expandedTargets = New-Object System.Collections.ArrayList

function Expand-Target {
    param($Condition, [string]$Label)
    $candidates = $window.FindAll($Scope::Descendants, $Condition)
    foreach ($candidate in $candidates) {
        try {
            $pattern = $candidate.GetCurrentPattern(
                [System.Windows.Automation.ExpandCollapsePattern]::Pattern)
            $pattern.Expand()
            Start-Sleep -Milliseconds 500
            [void]$expandedTargets.Add($candidate)
            Write-Output ('PROBE: expanded {0} state={1}' -f $Label, $pattern.Current.ExpandCollapseState)
            return
        } catch { }
    }
    Write-Output ('PROBE: expand-not-possible {0}' -f $Label)
}

if (-not [string]::IsNullOrEmpty($ExpandAid)) {
    $aidCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $ExpandAid)
    Expand-Target -Condition $aidCondition -Label ('aid="{0}"' -f $ExpandAid)
}

# Second hop: some flyouts (shape outline / fill / size) only exist once the shape
# gallery is open, and they have no AutomationId, so match on Name instead.
if (-not [string]::IsNullOrEmpty($ThenExpandName)) {
    $nameCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $ThenExpandName)
    Expand-Target -Condition $nameCondition -Label ('name="{0}"' -f $ThenExpandName)
}

$watch = [System.Diagnostics.Stopwatch]::StartNew()
$deadline = $watch.Elapsed.TotalSeconds + $DeadlineSeconds

$walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$queue = New-Object System.Collections.Queue
$queue.Enqueue(@($window, 0))

$seen = 0
$emitted = 0
$truncated = $false

while ($queue.Count -gt 0) {
    if ($watch.Elapsed.TotalSeconds -gt $deadline) { $truncated = $true; break }
    if ($seen -ge $MaxElements) { $truncated = $true; break }

    $item = $queue.Dequeue()
    $node = $item[0]
    $depth = $item[1]
    $seen++

    $line = Format-Element -Element $node
    if ([string]::IsNullOrEmpty($Filter) -or ($line -match $Filter)) {
        Write-Output ('[{0}] {1}' -f $depth, $line)
        $emitted++
    }

    if ($depth -ge $MaxDepth) { continue }

    $child = $walker.GetFirstChild($node)
    while ($null -ne $child) {
        $queue.Enqueue(@($child, ($depth + 1)))
        $child = $walker.GetNextSibling($child)
    }
}

$watch.Stop()
Write-Output ('PROBE: seen={0} emitted={1} truncated={2} elapsed_ms={3}' -f `
    $seen, $emitted, $truncated, [int]$watch.Elapsed.TotalMilliseconds)

# Leave the UI as we found it: collapse what this run opened, innermost first.
for ($i = $expandedTargets.Count - 1; $i -ge 0; $i--) {
    try {
        $restore = $expandedTargets[$i].GetCurrentPattern(
            [System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        $restore.Collapse()
        Start-Sleep -Milliseconds 250
        Write-Output ('PROBE: collapsed index={0} state={1}' -f $i, $restore.Current.ExpandCollapseState)
    } catch {
        Write-Output ('PROBE: collapse-failed index={0}' -f $i)
    }
}
