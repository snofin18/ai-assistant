# TASK-046 Paint T3.3 draw / undo / pixel-rollback runner.
#
# Boundaries:
#   - the caller must explicitly opt in to closing the test Paint instance;
#   - each run starts from a fresh Paint process and therefore a clean document;
#   - the script records pass/fail and raw test output, but never invents an
#     undo-step or pixel-difference measurement.
#
# Usage:
#   powershell -NoProfile -File eval/tasks/paint/t3.3/run-ten.ps1 -AllowClosePaint

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
    $OutputDirectory = Join-Path $repoRoot 'target/paint-t3-3-ten-run'
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PaintT33TenRunNative {
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
        $foreground = [PaintT33TenRunNative]::GetForegroundWindow()
        [uint32]$targetProcessId = 0
        [uint32]$foregroundProcessId = 0
        $targetThread = [PaintT33TenRunNative]::GetWindowThreadProcessId(
            $target,
            [ref]$targetProcessId
        )
        $foregroundThread = [PaintT33TenRunNative]::GetWindowThreadProcessId(
            $foreground,
            [ref]$foregroundProcessId
        )
        $currentThread = [PaintT33TenRunNative]::GetCurrentThreadId()
        $attachedCurrent = $false
        $attachedForeground = $false
        try {
            if ($currentThread -ne $targetThread) {
                $attachedCurrent = [PaintT33TenRunNative]::AttachThreadInput(
                    $currentThread,
                    $targetThread,
                    $true
                )
            }
            if ($foregroundThread -ne 0 -and $foregroundThread -ne $targetThread) {
                $attachedForeground = [PaintT33TenRunNative]::AttachThreadInput(
                    $foregroundThread,
                    $targetThread,
                    $true
                )
            }
            [void][PaintT33TenRunNative]::ShowWindow($target, 3)
            [void][PaintT33TenRunNative]::BringWindowToTop($target)
            [void][PaintT33TenRunNative]::SetForegroundWindow($target)
            [void][PaintT33TenRunNative]::SetFocus($target)
        } finally {
            if ($attachedForeground) {
                [void][PaintT33TenRunNative]::AttachThreadInput(
                    $foregroundThread,
                    $targetThread,
                    $false
                )
            }
            if ($attachedCurrent) {
                [void][PaintT33TenRunNative]::AttachThreadInput(
                    $currentThread,
                    $targetThread,
                    $false
                )
            }
        }
        Start-Sleep -Milliseconds 250
        if ([PaintT33TenRunNative]::GetForegroundWindow() -eq $PaintProcess.MainWindowHandle) {
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
                test_paint_t3_3_draw_undo_pixel_snapshot_verify `
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

$successRate = $passed / $Runs
$minimumSuccessRate = 0.95
$summary = [pscustomobject]@{
    task_id = 'paint.t3_3.draw_undo_pixel_snapshot_verify'
    requested_runs = $Runs
    passed_runs = $passed
    success_rate = [math]::Round($successRate, 4)
    minimum_success_rate = $minimumSuccessRate
    undo_success_rate = [math]::Round($successRate, 4)
    expected_undo_steps = 1
    undo_step_assertion_passed_runs = $passed
    pixel_rollback_assertion_passed_runs = $passed
    pixel_rollback_contract = 'mean_abs_diff_within(max=2, confidence_min=0.9)'
    runs = $records
}
$summaryPath = Join-Path $OutputDirectory 'ten-run-summary.json'
$summary | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding utf8
Write-Output "summary=$summaryPath"

if ($successRate -lt $minimumSuccessRate) {
    exit 1
}
