# TASK-045 Paint T3.2 real drag endpoint coordinate-error probe.
#
# The script runs the existing T3.2 production acceptance with a fresh Paint
# document, then immediately reads the physical cursor position. The expected
# drop point is computed from the live canvas bounds, document size, and the
# configured T3.2 end point (1984, 1488). No value is inferred from a pass/fail.
#
# Usage:
#   powershell -NoProfile -File eval/tasks/paint/t3.2/coordinate-error.ps1 -AllowClosePaint

[CmdletBinding()]
param(
    [string]$OutputDirectory = '',
    [switch]$AllowClosePaint
)

$ErrorActionPreference = 'Stop'

if (-not $AllowClosePaint) {
    throw 'Refusing to close Paint without -AllowClosePaint'
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'target/paint-t3-2-coordinate-error'
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PaintCoordinateErrorNative {
    [StructLayout(LayoutKind.Sequential)]
    public struct POINT {
        public int X;
        public int Y;
    }
    [DllImport("user32.dll")]
    public static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")]
    public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")]
    public static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool SetFocus(IntPtr hWnd);
    [DllImport("user32.dll")]
    public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
    [DllImport("kernel32.dll")]
    public static extern uint GetCurrentThreadId();
}
'@

[void][PaintCoordinateErrorNative]::SetProcessDpiAwarenessContext([IntPtr](-4))

function Close-PaintProcesses {
    $paintProcesses = @(Get-Process -Name mspaint -ErrorAction SilentlyContinue)
    foreach ($process in $paintProcesses) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 500
}

function Start-PaintWindow {
    Close-PaintProcesses
    Start-Process 'shell:AppsFolder\Microsoft.Paint_8wekyb3d8bbwe!App'
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline) {
        $paint = Get-Process -Name mspaint -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowHandle -ne 0 } |
            Sort-Object StartTime -Descending |
            Select-Object -First 1
        if ($null -ne $paint) {
            return $paint
        }
        Start-Sleep -Milliseconds 250
    }
    throw 'Paint window did not appear within 30 seconds'
}

function Focus-PaintWindow {
    param($PaintProcess)

    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline) {
        $target = $PaintProcess.MainWindowHandle
        $foreground = [PaintCoordinateErrorNative]::GetForegroundWindow()
        [uint32]$targetProcessId = 0
        [uint32]$foregroundProcessId = 0
        $targetThread = [PaintCoordinateErrorNative]::GetWindowThreadProcessId(
            $target,
            [ref]$targetProcessId
        )
        $foregroundThread = [PaintCoordinateErrorNative]::GetWindowThreadProcessId(
            $foreground,
            [ref]$foregroundProcessId
        )
        $currentThread = [PaintCoordinateErrorNative]::GetCurrentThreadId()
        $attachedCurrent = $false
        $attachedForeground = $false
        try {
            if ($currentThread -ne $targetThread) {
                $attachedCurrent = [PaintCoordinateErrorNative]::AttachThreadInput(
                    $currentThread,
                    $targetThread,
                    $true
                )
            }
            if ($foregroundThread -ne 0 -and $foregroundThread -ne $targetThread) {
                $attachedForeground = [PaintCoordinateErrorNative]::AttachThreadInput(
                    $foregroundThread,
                    $targetThread,
                    $true
                )
            }
            [void][PaintCoordinateErrorNative]::ShowWindow($target, 3)
            [void][PaintCoordinateErrorNative]::BringWindowToTop($target)
            [void][PaintCoordinateErrorNative]::SetForegroundWindow($target)
            [void][PaintCoordinateErrorNative]::SetFocus($target)
        } finally {
            if ($attachedForeground) {
                [void][PaintCoordinateErrorNative]::AttachThreadInput(
                    $foregroundThread,
                    $targetThread,
                    $false
                )
            }
            if ($attachedCurrent) {
                [void][PaintCoordinateErrorNative]::AttachThreadInput(
                    $currentThread,
                    $targetThread,
                    $false
                )
            }
        }
        Start-Sleep -Milliseconds 250
        if ([PaintCoordinateErrorNative]::GetForegroundWindow() -eq $target) {
            return
        }
    }
    throw 'Paint did not become the foreground window'
}

function Find-PaintElement {
    param($Window, [System.Windows.Automation.AutomationProperty]$Property, $Value)

    $condition = New-Object System.Windows.Automation.PropertyCondition($Property, $Value)
    return $Window.FindFirst(
        [System.Windows.Automation.TreeScope]::Descendants,
        $condition
    )
}

$paint = $null
$testPassed = $false
$testOutput = @()
$cursorX = $null
$cursorY = $null
try {
    $paint = Start-PaintWindow
    Focus-PaintWindow -PaintProcess $paint
    $previousErrorAction = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $testOutput = & cargo test `
            -p assistant-agent-core `
            --test production_paint `
            test_paint_t3_2_open_png_mark_region_save_as `
            -- --ignored --nocapture 2>&1 |
            ForEach-Object { $_.ToString() }
        $testPassed = $LASTEXITCODE -eq 0
    } finally {
        $ErrorActionPreference = $previousErrorAction
    }
    if (-not $testPassed) {
        throw 'T3.2 real-machine test failed; coordinate error was not measured'
    }

    $point = New-Object PaintCoordinateErrorNative+POINT
    if (-not [PaintCoordinateErrorNative]::GetCursorPos([ref]$point)) {
        throw 'GetCursorPos failed after the drag'
    }
    $cursorX = $point.X
    $cursorY = $point.Y

    $uiaWindow = Find-PaintElement `
        -Window ([System.Windows.Automation.AutomationElement]::RootElement) `
        -Property ([System.Windows.Automation.AutomationElement]::ProcessIdProperty) `
        -Value $paint.Id
    if ($null -eq $uiaWindow) {
        throw 'Paint UIA window was not found'
    }
    $canvas = Find-PaintElement `
        -Window $uiaWindow `
        -Property ([System.Windows.Automation.AutomationElement]::AutomationIdProperty) `
        -Value 'image'
    $sizeText = Find-PaintElement `
        -Window $uiaWindow `
        -Property ([System.Windows.Automation.AutomationElement]::AutomationIdProperty) `
        -Value 'CanvasSizeTextBlock'
    if ($null -eq $canvas -or $null -eq $sizeText) {
        throw 'Paint canvas bounds or size text was not found'
    }

    $canvasRect = $canvas.Current.BoundingRectangle
    $numbers = [regex]::Matches($sizeText.Current.Name, '\d+') |
        ForEach-Object { [int]$_.Value }
    if ($numbers.Count -lt 2 -or $numbers[0] -le 0) {
        throw "CanvasSizeTextBlock did not expose a document size: $($sizeText.Current.Name)"
    }
    $documentWidth = $numbers[0]
    $documentHeight = $numbers[1]
    if ($canvasRect.Width -le 0) {
        throw 'Canvas rendered width is not positive'
    }
    $zoom = [double]$canvasRect.Width / [double]$documentWidth
    $expectedX = [int][Math]::Round([double]$canvasRect.Left + ($zoom * 1984.0))
    $expectedY = [int][Math]::Round([double]$canvasRect.Top + ($zoom * 1488.0))
    $errorX = [Math]::Abs($cursorX - $expectedX)
    $errorY = [Math]::Abs($cursorY - $expectedY)
    $passed = $errorX -le 2 -and $errorY -le 2
    $summary = [pscustomobject]@{
        task_id = 'paint.t3_2.open_png_mark_region_save_as'
        configured_end_canvas = @{ x = 1984; y = 1488 }
        document_width_px = $documentWidth
        document_height_px = $documentHeight
        canvas_bounds = @{
            left = [int]$canvasRect.Left
            top = [int]$canvasRect.Top
            width = [int]$canvasRect.Width
            height = [int]$canvasRect.Height
        }
        zoom_ratio = $zoom
        expected_physical = @{ x = $expectedX; y = $expectedY }
        observed_cursor = @{ x = $cursorX; y = $cursorY }
        error_px = @{ x = $errorX; y = $errorY; max = ([Math]::Max($errorX, $errorY)) }
        tolerance_px = 2
        passed = $passed
        test_passed = $testPassed
    }
    $summary | ConvertTo-Json -Depth 6 | Set-Content `
        -LiteralPath (Join-Path $OutputDirectory 'coordinate-error-summary.json') `
        -Encoding utf8
    $testOutput | Set-Content `
        -LiteralPath (Join-Path $OutputDirectory 't3.2-real-test.log') `
        -Encoding utf8
    $summary | ConvertTo-Json -Depth 6
    if (-not $passed) {
        exit 1
    }
} finally {
    if ($null -ne $paint) {
        Stop-Process -Id $paint.Id -Force -ErrorAction SilentlyContinue
    } else {
        Close-PaintProcesses
    }
}
