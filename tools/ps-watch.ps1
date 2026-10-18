<#
.SYNOPSIS
  Track PowerShell processes and capture stalls (agent-side diagnostics).

.DESCRIPTION
  Appends one JSONL `sample` per `pwsh` / `powershell` process every -IntervalSeconds to
  `$env:USERPROFILE\.codex\ps-watch\<yyyy-MM-dd>.jsonl`. A process whose total CPU does not
  advance for -StallSamples consecutive samples is *classified by its command line* before being
  reported: a process whose command line contains `Start-Sleep` is recorded as `waiting` (a
  deliberate zero-CPU wait), everything else becomes a `stall` event carrying its command line,
  handle / thread counts and per-thread wait reasons, so an intermittent freeze leaves evidence
  instead of only "no output for 90 s".

  Why the classification exists: on 2026-10-16 all 13 recorded `stall` events turned out to be
  `Start-Sleep` waits, because "total CPU did not advance" is identical for a sleep and for a
  freeze. Without this split the counter is unusable as evidence.

  Why a separate file: the agent's shell is a bundled runtime pwsh; the hang is intermittent and
  so far only observable from a *second* process. Sample lines are intentionally tiny so the
  tracker stays cheap; the heavy detail is only written for a suspected stall.

  Run detached and hidden (see docs/automations/codex-automations-operations.md, section 4.1.d):
    Start-Process pwsh -WindowStyle Hidden -ArgumentList '-NoProfile','-NonInteractive','-File','<this file>'

.PARAMETER IntervalSeconds
  Sampling period. Default 3 s.

.PARAMETER StallSamples
  Consecutive samples with unchanged total CPU before a `stall` event is emitted. Default 20 (~60 s).

.PARAMETER LogDirectory
  Output directory. Default `%USERPROFILE%\.codex\ps-watch`.

.PARAMETER Once
  Take a single sample and exit (smoke test).
#>
[CmdletBinding()]
param(
    [int]$IntervalSeconds = 3,
    [int]$StallSamples = 20,
    [string]$LogDirectory = (Join-Path $env:USERPROFILE '.codex\ps-watch'),
    [switch]$Once
)

$ErrorActionPreference = 'Continue'
Set-StrictMode -Off

if (-not (Test-Path -LiteralPath $LogDirectory)) {
    New-Item -ItemType Directory -Force -Path $LogDirectory | Out-Null
}
$logFile = Join-Path $LogDirectory ("{0}.jsonl" -f (Get-Date -Format 'yyyy-MM-dd'))

function Write-WatchEvent {
    param([hashtable]$Fields)
    $Fields['ts'] = (Get-Date).ToString('o')
    $line = ($Fields | ConvertTo-Json -Compress -Depth 4)
    Add-Content -LiteralPath $logFile -Value $line -Encoding UTF8
}

Write-WatchEvent @{
    event         = 'watchdog_start'
    watchdog_pid  = $PID
    interval_s    = $IntervalSeconds
    stall_samples = $StallSamples
    log_file      = $logFile
}

$cpuHistory = @{}

do {
    $processes = @(Get-Process -Name pwsh, powershell -ErrorAction SilentlyContinue)
    $seen = @{}

    foreach ($process in $processes) {
        $seen[$process.Id] = $true
        $cpu = $null; $started = $null; $threads = $null; $handles = $null; $wsMb = $null
        try { $cpu = [math]::Round($process.CPU, 2) } catch { }
        try { $started = $process.StartTime.ToString('o') } catch { }
        try { $threads = $process.Threads.Count } catch { }
        try { $handles = $process.HandleCount } catch { }
        try { $wsMb = [math]::Round($process.WorkingSet64 / 1MB, 1) } catch { }

        Write-WatchEvent @{
            event   = 'sample'
            pid     = $process.Id
            name    = $process.ProcessName
            started = $started
            cpu_s   = $cpu
            threads = $threads
            handles = $handles
            ws_mb   = $wsMb
        }

        if ($null -eq $cpu) { continue }

        if (-not $cpuHistory.ContainsKey($process.Id)) {
            $cpuHistory[$process.Id] = @{ cpu = $cpu; flat = 0; reported = $false }
            continue
        }

        $state = $cpuHistory[$process.Id]
        if ($state.cpu -eq $cpu) { $state.flat += 1 } else { $state.flat = 0; $state.reported = $false }
        $state.cpu = $cpu

        if ($state.flat -ge $StallSamples -and -not $state.reported) {
            # Classify BEFORE reporting: flat CPU alone cannot tell a deliberate wait from a
            # freeze, so the command line decides. Resolution order for CommandLine can be slow,
            # hence it is only fetched on the (rare) threshold crossing, not on every sample.
            $commandLine = $null
            try {
                $commandLine = (Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)").CommandLine
            } catch { }

            if ($null -ne $commandLine -and $commandLine -match 'Start-Sleep') {
                Write-WatchEvent @{
                    event        = 'waiting'
                    pid          = $process.Id
                    name         = $process.ProcessName
                    flat_samples = $state.flat
                    cpu_s        = $cpu
                    ws_mb        = $wsMb
                    command_line = $commandLine
                }
            } else {
                $threadStates = $null
                try {
                    $threadStates = ($process.Threads | ForEach-Object { "{0}/{1}" -f $_.ThreadState, $_.WaitReason }) -join ','
                } catch { }
                Write-WatchEvent @{
                    event         = 'stall'
                    pid           = $process.Id
                    name          = $process.ProcessName
                    flat_samples  = $state.flat
                    cpu_s         = $cpu
                    threads       = $threads
                    handles       = $handles
                    ws_mb         = $wsMb
                    command_line  = $commandLine
                    thread_states = $threadStates
                    classified_by = 'command_line'
                }
            }
            $state.reported = $true
        }
    }

    foreach ($staleId in @($cpuHistory.Keys)) {
        if (-not $seen.ContainsKey($staleId)) { $cpuHistory.Remove($staleId) }
    }

    if ($Once) { break }
    Start-Sleep -Seconds $IntervalSeconds
} while ($true)

Write-WatchEvent @{ event = 'watchdog_stop'; watchdog_pid = $PID }
