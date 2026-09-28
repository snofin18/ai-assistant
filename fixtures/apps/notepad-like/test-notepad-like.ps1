Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "notepad-like.ps1"
$manifestPath = Join-Path $PSScriptRoot "automation-ids.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$requiredIds = @($manifest.required)
$runtimeIds = @($manifest.runtime)
$allIds = @($requiredIds + $runtimeIds)
$faultModes = @("none", "disappear", "timeout", "ambiguous", "dialog", "busy")
$powershellPath = Join-Path $PSHOME "powershell.exe"
$failures = New-Object System.Collections.Generic.List[string]

function Remove-TestDirectory {
    param([string]$Path)
    $resolved = [IO.Path]::GetFullPath($Path)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove non-temp path: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

function Wait-StateFile {
    param(
        [string]$Path,
        [int]$TimeoutSeconds = 8
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path -LiteralPath $Path) {
            return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
        }
        Start-Sleep -Milliseconds 100
    }
    throw "State file was not created within $TimeoutSeconds seconds: $Path"
}

foreach ($mode in $faultModes) {
    $testDirectory = Join-Path ([IO.Path]::GetTempPath()) ("notepad-like-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $testDirectory | Out-Null
    $stateFile = Join-Path $testDirectory "state.json"
    $process = $null

    try {
        $process = Start-Process -FilePath $powershellPath `
            -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Sta", "-File", $scriptPath,
                "--fault", $mode, "--state-file", $stateFile, "--auto-close-ms", "1500") `
            -PassThru -WindowStyle Normal

        $state = Wait-StateFile -Path $stateFile
        if ($state.schema_version -ne "1.0") {
            throw "Unexpected schema_version: $($state.schema_version)"
        }
        if ($state.app -ne "notepad-like") {
            throw "Unexpected app id: $($state.app)"
        }
        if ($state.fault -ne $mode) {
            throw "Expected fault $mode, got $($state.fault)"
        }
        if ($mode -eq "none" -and $state.status -ne "ready") {
            throw "Expected ready status for none, got $($state.status)"
        }
        if ($mode -ne "none" -and $state.status -ne "fault_applied") {
            throw "Expected fault_applied status for $mode, got $($state.status)"
        }
        foreach ($requiredId in $allIds) {
            if ($state.automation_ids -notcontains $requiredId) {
                throw "State file missing AutomationId: $requiredId"
            }
        }
        Write-Host "PASS fault=$mode"
    } catch {
        $failures.Add("fault=$mode : " + $_.Exception.Message) | Out-Null
        Write-Host "FAIL fault=$mode : $($_.Exception.Message)"
    } finally {
        if ($null -ne $process -and -not $process.HasExited) {
            Stop-Process -Id $process.Id -Force
        }
        Remove-TestDirectory -Path $testDirectory
    }
}

if ($failures.Count -gt 0) {
    Write-Host "notepad-like tests failed:"
    foreach ($failure in $failures) {
        Write-Host " - $failure"
    }
    exit 1
}

Write-Host "notepad-like tests passed."
exit 0
