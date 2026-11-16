# Dedicated Edge process-tree cleanup for the T5.2 Windows acceptance runner.
#
# This helper is dot-sourced by the runner and by its process-tree regression test.
# It deliberately scopes process matching to a unique profile path, treats taskkill
# exit codes as diagnostics, and uses the remaining profile processes as the
# authoritative cleanup postcondition. Related: ADR-0063, TASK-369, TASK-371.

function Get-DedicatedEdgeProcesses {
    param(
        [Parameter(Mandatory = $true)][string]$Profile,
        [string]$ProcessName = "msedge.exe"
    )

    if ([string]::IsNullOrWhiteSpace($Profile)) {
        throw "Profile must be non-empty for dedicated Edge process matching"
    }
    if ($ProcessName -notmatch "^[A-Za-z0-9._-]+$") {
        throw "ProcessName contains unsupported characters: $ProcessName"
    }
    $escapedProcessName = $ProcessName -replace "'", "''"
    return @(
        Get-CimInstance Win32_Process -Filter "Name='$escapedProcessName'" |
            Where-Object {
                $_.CommandLine -and
                $_.CommandLine.IndexOf(
                    $Profile,
                    [System.StringComparison]::OrdinalIgnoreCase
                ) -ge 0
            }
    )
}

function Write-DedicatedEdgeCleanupLog {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Profile,
        [Parameter(Mandatory = $true)][string]$ProcessName,
        [Parameter(Mandatory = $true)][int]$InitialProcessCount,
        [Parameter(Mandatory = $true)][int]$TaskkillAttemptCount,
        [Parameter(Mandatory = $true)][int]$RemainingProcessCount,
        [Parameter(Mandatory = $true)][int]$ElapsedMilliseconds,
        [Parameter(Mandatory = $true)][string]$Result,
        [string[]]$TaskkillExitCodes = @(),
        [int[]]$RemainingProcessIds = @(),
        [string]$Failure = ""
    )

    $directory = [System.IO.Path]::GetDirectoryName($Path)
    if (-not [string]::IsNullOrWhiteSpace($directory)) {
        [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    }
    $text = @(
        "profile=$Profile",
        "process_name=$ProcessName",
        "initial_process_count=$InitialProcessCount",
        "taskkill_attempt_count=$TaskkillAttemptCount",
        "taskkill_exit_codes=$($TaskkillExitCodes -join ',')",
        "remaining_process_count=$RemainingProcessCount",
        "remaining_process_ids=$($RemainingProcessIds -join ',')",
        "elapsed_ms=$ElapsedMilliseconds",
        "result=$Result",
        "failure=$Failure"
    ) -join "`n"
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $text + "`n", $encoding)
}

function Stop-DedicatedEdgeProcessTree {
    param(
        [Parameter(Mandatory = $true)][string]$Profile,
        [string]$ProcessName = "msedge.exe",
        [ValidateRange(1, 60)][int]$TimeoutSeconds = 15,
        [ValidateRange(1, 128)][int]$MaximumTaskkillAttempts = 32,
        [ValidateRange(1, 1024)][int]$MaximumMatchedProcesses = 256,
        [string]$LogPath = ""
    )

    if ([string]::IsNullOrWhiteSpace($Profile)) {
        throw "Profile must be non-empty before dedicated Edge cleanup"
    }

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $initialProcesses = @(
        Get-DedicatedEdgeProcesses -Profile $Profile -ProcessName $ProcessName
    )
    $initialCount = $initialProcesses.Count
    if ($initialCount -gt $MaximumMatchedProcesses) {
        $message = "Refusing cleanup: profile matched $initialCount processes, limit is $MaximumMatchedProcesses"
        if (-not [string]::IsNullOrWhiteSpace($LogPath)) {
            Write-DedicatedEdgeCleanupLog `
                -Path $LogPath `
                -Profile $Profile `
                -ProcessName $ProcessName `
                -InitialProcessCount $initialCount `
                -TaskkillAttemptCount 0 `
                -RemainingProcessCount $initialCount `
                -ElapsedMilliseconds $stopwatch.ElapsedMilliseconds `
                -Result "failed" `
                -RemainingProcessIds @($initialProcesses | ForEach-Object { $_.ProcessId }) `
                -Failure $message
        }
        throw $message
    }

    $taskkillExitCodes = New-Object System.Collections.Generic.List[string]
    $spawnFailure = ""
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ($true) {
        $matchingProcesses = @(
            Get-DedicatedEdgeProcesses -Profile $Profile -ProcessName $ProcessName
        )
        if ($matchingProcesses.Count -eq 0) {
            break
        }
        if (
            $taskkillExitCodes.Count -ge $MaximumTaskkillAttempts -or
            (Get-Date) -ge $deadline
        ) {
            break
        }

        foreach ($process in $matchingProcesses) {
            if ($taskkillExitCodes.Count -ge $MaximumTaskkillAttempts) {
                break
            }
            try {
                $taskkill = Start-Process `
                    -FilePath "taskkill.exe" `
                    -ArgumentList @("/PID", "$($process.ProcessId)", "/T", "/F") `
                    -WindowStyle Hidden `
                    -Wait `
                    -PassThru
                $taskkillExitCodes.Add([string]$taskkill.ExitCode)
            } catch {
                $spawnFailure = $_.Exception.Message
                break
            }
        }
        if (-not [string]::IsNullOrWhiteSpace($spawnFailure)) {
            break
        }
        Start-Sleep -Milliseconds 250
    }

    $remainingProcesses = @(
        Get-DedicatedEdgeProcesses -Profile $Profile -ProcessName $ProcessName
    )
    $remainingCount = $remainingProcesses.Count
    $result = if ($remainingCount -eq 0 -and [string]::IsNullOrWhiteSpace($spawnFailure)) {
        "passed"
    } else {
        "failed"
    }
    $failure = if ([string]::IsNullOrWhiteSpace($spawnFailure)) {
        if ($remainingCount -eq 0) {
            ""
        } else {
            "Dedicated Edge process tree did not converge within ${TimeoutSeconds}s or $MaximumTaskkillAttempts taskkill attempts"
        }
    } else {
        "taskkill spawn failed: $spawnFailure"
    }

    if (-not [string]::IsNullOrWhiteSpace($LogPath)) {
        Write-DedicatedEdgeCleanupLog `
            -Path $LogPath `
            -Profile $Profile `
            -ProcessName $ProcessName `
            -InitialProcessCount $initialCount `
            -TaskkillAttemptCount $taskkillExitCodes.Count `
            -RemainingProcessCount $remainingCount `
            -ElapsedMilliseconds $stopwatch.ElapsedMilliseconds `
            -Result $result `
            -TaskkillExitCodes $taskkillExitCodes.ToArray() `
            -RemainingProcessIds @($remainingProcesses | ForEach-Object { $_.ProcessId }) `
            -Failure $failure
    }

    if ($result -ne "passed") {
        throw $failure
    }

    return [pscustomobject]@{
        profile = $Profile
        process_name = $ProcessName
        initial_process_count = $initialCount
        taskkill_attempt_count = $taskkillExitCodes.Count
        remaining_process_count = $remainingCount
        elapsed_ms = $stopwatch.ElapsedMilliseconds
        result = $result
    }
}
