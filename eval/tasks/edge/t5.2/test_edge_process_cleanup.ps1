$ErrorActionPreference = "Stop"
$helperPath = Join-Path $PSScriptRoot "edge_process_cleanup.ps1"
$fixturePath = Join-Path $PSScriptRoot "edge_process_tree_fixture.ps1"
if (-not (Test-Path -LiteralPath $helperPath -PathType Leaf)) {
    throw "Missing cleanup helper: $helperPath"
}
if (-not (Test-Path -LiteralPath $fixturePath -PathType Leaf)) {
    throw "Missing process-tree fixture: $fixturePath"
}
. $helperPath

$pwshPath = (Get-Command pwsh -ErrorAction Stop).Source
$processName = Split-Path -Leaf $pwshPath
$failures = New-Object System.Collections.Generic.List[string]

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) {
        $failures.Add($Message)
    }
}

function Wait-ProfileProcessCount {
    param(
        [Parameter(Mandatory = $true)][string]$Profile,
        [Parameter(Mandatory = $true)][int]$MinimumCount
    )
    for ($attempt = 0; $attempt -lt 60; $attempt++) {
        $processes = @(
            Get-DedicatedEdgeProcesses -Profile $Profile -ProcessName $processName
        )
        if ($processes.Count -ge $MinimumCount) {
            return $processes
        }
        Start-Sleep -Milliseconds 250
    }
    throw "Process-tree fixture did not reach $MinimumCount matching processes"
}

function Start-ProcessTreeFixture {
    param([Parameter(Mandatory = $true)][string]$Marker)
    return Start-Process `
        -FilePath $pwshPath `
        -ArgumentList @(
            "-NoProfile",
            "-File",
            $fixturePath,
            "-Marker",
            $Marker
        ) `
        -WindowStyle Hidden `
        -PassThru
}

$firstMarker = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("task-371-cleanup-" + [Guid]::NewGuid().ToString("N"))
$protectedMarker = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("task-371-protected-" + [Guid]::NewGuid().ToString("N"))
$cleanupLogPath = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("task-371-cleanup-" + [Guid]::NewGuid().ToString("N") + ".log")
$firstRoot = $null
$protectedRoot = $null

try {
    $firstRoot = Start-ProcessTreeFixture -Marker $firstMarker
    $firstProcesses = Wait-ProfileProcessCount `
        -Profile $firstMarker `
        -MinimumCount 2
    $firstResult = Stop-DedicatedEdgeProcessTree `
        -Profile $firstMarker `
        -ProcessName $processName `
        -TimeoutSeconds 10 `
        -LogPath $cleanupLogPath
    $firstRemaining = @(
        Get-DedicatedEdgeProcesses -Profile $firstMarker -ProcessName $processName
    )

    Assert-Condition `
        -Condition ($firstProcesses.Count -ge 2) `
        -Message "fixture did not create a parent/child process tree"
    Assert-Condition `
        -Condition ($firstResult.initial_process_count -ge 2) `
        -Message "cleanup did not observe the complete process tree"
    Assert-Condition `
        -Condition ($firstResult.taskkill_attempt_count -ge 1) `
        -Message "cleanup did not invoke taskkill"
    Assert-Condition `
        -Condition ($firstResult.remaining_process_count -eq 0) `
        -Message "cleanup result reported remaining processes"
    Assert-Condition `
        -Condition ($firstRemaining.Count -eq 0) `
        -Message "matching process tree survived cleanup"
    $cleanupLog = if (Test-Path -LiteralPath $cleanupLogPath -PathType Leaf) {
        Get-Content -LiteralPath $cleanupLogPath -Raw
    } else {
        ""
    }
    Assert-Condition `
        -Condition ($cleanupLog -match "(?m)^result=passed$") `
        -Message "cleanup log did not record a passed postcondition"
    Assert-Condition `
        -Condition ($cleanupLog -match "(?m)^remaining_process_count=0$") `
        -Message "cleanup log did not record zero remaining processes"

    $emptyProfileRejected = $false
    try {
        Stop-DedicatedEdgeProcessTree `
            -Profile " " `
            -ProcessName $processName `
            -TimeoutSeconds 1 | Out-Null
    } catch {
        $emptyProfileRejected = $true
    }
    Assert-Condition `
        -Condition $emptyProfileRejected `
        -Message "empty profile was not rejected"

    $protectedRoot = Start-ProcessTreeFixture -Marker $protectedMarker
    $protectedProcesses = Wait-ProfileProcessCount `
        -Profile $protectedMarker `
        -MinimumCount 2
    $nonMatchingResult = Stop-DedicatedEdgeProcessTree `
        -Profile (Join-Path ([System.IO.Path]::GetTempPath()) "task-371-no-match") `
        -ProcessName $processName `
        -TimeoutSeconds 2
    $protectedRemaining = @(
        Get-DedicatedEdgeProcesses -Profile $protectedMarker -ProcessName $processName
    )
    Assert-Condition `
        -Condition ($nonMatchingResult.initial_process_count -eq 0) `
        -Message "non-matching cleanup unexpectedly found processes"
    Assert-Condition `
        -Condition ($protectedRemaining.Count -eq $protectedProcesses.Count) `
        -Message "profile-scoped cleanup changed an unrelated process tree"
} finally {
    if (Test-Path -LiteralPath $cleanupLogPath -PathType Leaf) {
        Remove-Item -LiteralPath $cleanupLogPath -Force
    }
    foreach ($marker in @($firstMarker, $protectedMarker)) {
        if (-not [string]::IsNullOrWhiteSpace($marker)) {
            try {
                Stop-DedicatedEdgeProcessTree `
                    -Profile $marker `
                    -ProcessName $processName `
                    -TimeoutSeconds 10 | Out-Null
            } catch {
                $failures.Add("test cleanup failed for ${marker}: $($_.Exception.Message)")
            }
        }
    }
}

if ($failures.Count -ne 0) {
    foreach ($failure in $failures) {
        Write-Output "FAIL: $failure"
    }
    exit 1
}

Write-Output "PASS: profile-scoped process-tree cleanup converged without collateral termination"
exit 0
