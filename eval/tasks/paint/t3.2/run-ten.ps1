# TASK-045 Paint T3.2 ten-run acceptance runner.
#
# Boundaries:
#   - the caller must explicitly opt in to closing the test Paint instance;
#   - each run starts from a fresh Paint process and therefore a clean document;
#   - the script records pass/fail and raw test output, but never invents a
#     coordinate-error measurement.
#
# Usage:
#   powershell -NoProfile -File eval/tasks/paint/t3.2/run-ten.ps1 -AllowClosePaint

[CmdletBinding()]
param(
    [int]$Runs = 10,
    [string]$OutputDirectory = '',
    [switch]$AllowClosePaint
)

$ErrorActionPreference = 'Stop'

if ($Runs -lt 1) {
    throw 'Runs must be at least 1'
}
if (-not $AllowClosePaint) {
    throw 'Refusing to close Paint without -AllowClosePaint'
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'target/paint-t3-2-ten-run'
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PaintTenRunNative {
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
        $foreground = [PaintTenRunNative]::GetForegroundWindow()
        [uint32]$targetProcessId = 0
        [uint32]$foregroundProcessId = 0
        $targetThread = [PaintTenRunNative]::GetWindowThreadProcessId(
            $target,
            [ref]$targetProcessId
        )
        $foregroundThread = [PaintTenRunNative]::GetWindowThreadProcessId(
            $foreground,
            [ref]$foregroundProcessId
        )
        $currentThread = [PaintTenRunNative]::GetCurrentThreadId()
        $attachedCurrent = $false
        $attachedForeground = $false
        try {
            if ($currentThread -ne $targetThread) {
                $attachedCurrent = [PaintTenRunNative]::AttachThreadInput(
                    $currentThread,
                    $targetThread,
                    $true
                )
            }
            if ($foregroundThread -ne 0 -and $foregroundThread -ne $targetThread) {
                $attachedForeground = [PaintTenRunNative]::AttachThreadInput(
                    $foregroundThread,
                    $targetThread,
                    $true
                )
            }
            [void][PaintTenRunNative]::ShowWindow($target, 3)
            [void][PaintTenRunNative]::BringWindowToTop($target)
            [void][PaintTenRunNative]::SetForegroundWindow($target)
            [void][PaintTenRunNative]::SetFocus($target)
        } finally {
            if ($attachedForeground) {
                [void][PaintTenRunNative]::AttachThreadInput(
                    $foregroundThread,
                    $targetThread,
                    $false
                )
            }
            if ($attachedCurrent) {
                [void][PaintTenRunNative]::AttachThreadInput(
                    $currentThread,
                    $targetThread,
                    $false
                )
            }
        }
        Start-Sleep -Milliseconds 250
        if ([PaintTenRunNative]::GetForegroundWindow() -eq $PaintProcess.MainWindowHandle) {
            return
        }
    }
    throw 'Paint did not become the foreground window'
}

$records = New-Object System.Collections.ArrayList
$passed = 0

for ($run = 1; $run -le $Runs; $run++) {
    $paint = $null
    $exitCode = 1
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $logPath = Join-Path $OutputDirectory ('run-{0:D2}.log' -f $run)

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
            $exitCode = $LASTEXITCODE
        } finally {
            $ErrorActionPreference = $previousErrorAction
        }
        $testOutput | Set-Content -LiteralPath $logPath -Encoding utf8
    } catch {
        $_ | Out-String | Set-Content -LiteralPath $logPath -Encoding utf8
    } finally {
        $stopwatch.Stop()
        if ($null -ne $paint) {
            Stop-Process -Id $paint.Id -Force -ErrorAction SilentlyContinue
        } else {
            Close-PaintProcesses
        }
    }

    $didPass = $exitCode -eq 0
    if ($didPass) {
        $passed++
    }
    [void]$records.Add([pscustomobject]@{
        run = $run
        passed = $didPass
        exit_code = $exitCode
        elapsed_ms = $stopwatch.ElapsedMilliseconds
        log = $logPath
    })
    Write-Output ("run={0} passed={1} exit={2} elapsed_ms={3}" -f `
        $run, $didPass, $exitCode, $stopwatch.ElapsedMilliseconds)
}

$summary = [pscustomobject]@{
    task_id = 'paint.t3_2.open_png_mark_region_save_as'
    requested_runs = $Runs
    passed_runs = $passed
    success_rate = [math]::Round($passed / $Runs, 4)
    minimum_success_rate = 0.75
    coordinate_error_px = 'not_measured'
    runs = $records
}
$summaryPath = Join-Path $OutputDirectory 'ten-run-summary.json'
$summary | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output "summary=$summaryPath"

if ($passed -lt [math]::Ceiling($Runs * 0.75)) {
    exit 1
}
