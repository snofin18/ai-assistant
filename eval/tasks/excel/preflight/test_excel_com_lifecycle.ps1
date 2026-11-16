param(
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [ValidateRange(5, 120)]
    [int]$CleanupTimeoutSeconds = 30,

    [ValidateRange(15, 300)]
    [int]$ProbeTimeoutSeconds = 90,

    [ValidateRange(15, 300)]
    [int]$BootstrapTimeoutSeconds = 90,

    [switch]$BootstrapHelper,

    [AllowNull()][string]$BootstrapOwnershipPath = $null,

    [AllowNull()][string]$BootstrapReadySignalPath = $null,

    [AllowNull()][string]$BootstrapStopSignalPath = $null
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$probePath = Join-Path $PSScriptRoot "probe_excel_com.ps1"
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
$requiredEvidencePrefix = [System.IO.Path]::GetFullPath(
    (Join-Path $repoRoot "eval\tasks\excel\preflight\evidence")
)
if (-not $evidence.StartsWith(
    $requiredEvidencePrefix,
    [System.StringComparison]::OrdinalIgnoreCase
)) {
    throw "EvidenceRoot must stay under eval/tasks/excel/preflight/evidence"
}
New-Item -ItemType Directory -Force -Path $evidence | Out-Null

$pwshCommand = Get-Command pwsh.exe -ErrorAction Stop
$pwshPath = $pwshCommand.Source
$startedAt = (Get-Date).ToUniversalTime().ToString("o")

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [AllowEmptyString()][string]$Text
    )
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Text, $encoding)
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    $text = (($Value | ConvertTo-Json -Depth 16) + "`n") -replace "`r`n", "`n"
    Write-Utf8File -Path $Path -Text $text
}

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) {
        throw $Message
    }
}

function Get-ExcelProcesses {
    return @(Get-Process EXCEL -ErrorAction SilentlyContinue)
}

if (-not ("ExcelComLifecycleNative" -as [type])) {
    Add-Type @"
using System;
using System.Runtime.InteropServices;

public static class ExcelComLifecycleNative
{
    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint GetWindowThreadProcessId(IntPtr windowHandle, out uint processId);
}
"@
}

function Get-ExcelApplicationProcessId {
    param([Parameter(Mandatory = $true)]$Excel)
    $windowHandle = [IntPtr]([long]$Excel.Hwnd)
    if ($windowHandle -eq [IntPtr]::Zero) {
        throw "Excel Application.Hwnd was zero; process ownership cannot be established"
    }
    [uint32]$processId = 0
    [void][ExcelComLifecycleNative]::GetWindowThreadProcessId(
        $windowHandle,
        [ref]$processId
    )
    if ($processId -eq 0) {
        throw "GetWindowThreadProcessId returned PID 0 for the Excel application"
    }
    return [int]$processId
}

function Get-ProcessIdentity {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return $null
    }
    $executablePath = try {
        [string]$process.Path
    } catch {
        $null
    }
    return [pscustomobject]@{
        id = [int]$process.Id
        process_name = [string]$process.ProcessName
        start_time_utc = $process.StartTime.ToUniversalTime().ToString("o")
        start_time_utc_ticks = [long]$process.StartTime.ToUniversalTime().Ticks
        executable_path = $executablePath
    }
}

function Test-ProcessIdentity {
    param([AllowNull()]$Identity)
    if ($null -eq $Identity) {
        return $false
    }
    $process = Get-Process -Id ([int]$Identity.id) -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        return $false
    }
    if ([string]$process.ProcessName -ne [string]$Identity.process_name) {
        return $false
    }
    $actualStartTimeTicks = [long]$process.StartTime.ToUniversalTime().Ticks
    if ($actualStartTimeTicks -ne [long]$Identity.start_time_utc_ticks) {
        return $false
    }
    if (-not [string]::IsNullOrWhiteSpace([string]$Identity.executable_path)) {
        $actualPath = try {
            [string]$process.Path
        } catch {
            $null
        }
        if ($actualPath -ne [string]$Identity.executable_path) {
            return $false
        }
    }
    return $true
}

function Invoke-VerifiedProcessTreeTermination {
    param([AllowNull()]$Identity)
    if (-not (Test-ProcessIdentity -Identity $Identity)) {
        return [pscustomobject]@{
            attempted = $false
            exit_code = $null
            error = "Process identity changed or process already exited"
        }
    }
    try {
        $taskkill = Start-Process `
            -FilePath "taskkill.exe" `
            -ArgumentList @("/PID", "$($Identity.id)", "/T", "/F") `
            -WindowStyle Hidden `
            -PassThru
        if (-not $taskkill.WaitForExit(10000)) {
            try {
                $taskkill.Kill()
            } catch {
            }
            return [pscustomobject]@{
                attempted = $true
                exit_code = $null
                error = "taskkill did not exit within 10 seconds"
            }
        }
        return [pscustomobject]@{
            attempted = $true
            exit_code = [string]$taskkill.ExitCode
            error = $null
        }
    } catch {
        return [pscustomobject]@{
            attempted = $true
            exit_code = $null
            error = $_.Exception.Message
        }
    }
}

function Get-ProcessIds {
    param([object[]]$Processes)
    return @($Processes | ForEach-Object { [int]$_.Id })
}

function Wait-ProcessIdentitiesGone {
    param(
        [AllowEmptyCollection()][object[]]$Identities,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )
    for ($attempt = 0; $attempt -lt ($TimeoutSeconds * 4); $attempt++) {
        $remaining = @(
            $Identities |
                Where-Object { Test-ProcessIdentity -Identity $_ }
        )
        if ($remaining.Count -eq 0) {
            return @()
        }
        Start-Sleep -Milliseconds 250
    }
    return @(
        $Identities |
            Where-Object { Test-ProcessIdentity -Identity $_ }
    )
}

function Wait-ExcelProcessCountZero {
    param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
    for ($attempt = 0; $attempt -lt ($TimeoutSeconds * 4); $attempt++) {
        $remaining = Get-ExcelProcesses
        if ($remaining.Count -eq 0) {
            return 0
        }
        Start-Sleep -Milliseconds 250
    }
    return (Get-ExcelProcesses).Count
}

function Release-ComObject {
    param(
        [AllowNull()]$ComObject,
        [Parameter(Mandatory = $true)][string]$Name
    )
    if ($null -eq $ComObject) {
        return $null
    }
    try {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($ComObject)
        return $null
    } catch {
        return "${Name} FinalReleaseComObject failed: $($_.Exception.Message)"
    }
}

function Invoke-ProbeScenario {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][int]$ExpectedExitCode,
        [string[]]$AdditionalArguments = @()
    )
    $scenarioDirectory = Join-Path $evidence $Name
    New-Item -ItemType Directory -Force -Path $scenarioDirectory | Out-Null
    $relativeEvidence = $scenarioDirectory.Substring($repoRoot.Length).TrimStart("\", "/")
    $arguments = @(
        "-NoProfile",
        "-File",
        $probePath,
        "-EvidenceRoot",
        $relativeEvidence,
        "-CleanupTimeoutSeconds",
        [string]$CleanupTimeoutSeconds
    ) + $AdditionalArguments

    $outputPath = Join-Path $scenarioDirectory "console.log"
    $stdoutPath = Join-Path $scenarioDirectory "stdout.log"
    $stderrPath = Join-Path $scenarioDirectory "stderr.log"
    if (Test-Path -LiteralPath $stdoutPath) {
        Remove-Item -LiteralPath $stdoutPath -Force
    }
    if (Test-Path -LiteralPath $stderrPath) {
        Remove-Item -LiteralPath $stderrPath -Force
    }
    $child = Start-Process `
        -FilePath $pwshPath `
        -ArgumentList $arguments `
        -WindowStyle Hidden `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath `
        -PassThru
    $childIdentity = Get-ProcessIdentity -ProcessId ([int]$child.Id)
    $completed = $child.WaitForExit($ProbeTimeoutSeconds * 1000)

    if (-not $completed) {
        $ownershipPath = Join-Path $scenarioDirectory "ownership.json"
        $ownedCleanup = $null
        if (Test-Path -LiteralPath $ownershipPath) {
            $ownership = Get-Content -LiteralPath $ownershipPath -Raw -Encoding utf8 |
                ConvertFrom-Json
            if ($null -ne $ownership.process_identity) {
                $ownedCleanup = Invoke-VerifiedProcessTreeTermination `
                    -Identity $ownership.process_identity
            }
        }
        $childCleanup = Invoke-VerifiedProcessTreeTermination -Identity $childIdentity
        throw (
            "Scenario ${Name} exceeded ${ProbeTimeoutSeconds}s; " +
            "owned_cleanup=$($ownedCleanup | ConvertTo-Json -Compress) " +
            "child_cleanup=$($childCleanup | ConvertTo-Json -Compress)"
        )
    }
    $child.WaitForExit()
    $exitCode = [int]$child.ExitCode
    $stdout = if (Test-Path -LiteralPath $stdoutPath) {
        Get-Content -LiteralPath $stdoutPath -Raw -Encoding utf8
    } else {
        ""
    }
    $stderr = if (Test-Path -LiteralPath $stderrPath) {
        Get-Content -LiteralPath $stderrPath -Raw -Encoding utf8
    } else {
        ""
    }
    $normalizedStdout = "$stdout".TrimEnd()
    $normalizedStderr = "$stderr".TrimEnd()
    $consoleText = @(
        "stdout:",
        $normalizedStdout,
        "stderr:",
        $normalizedStderr
    ) -join "`n"
    Write-Utf8File -Path $outputPath -Text ($consoleText.TrimEnd() + "`n")

    $summaryPath = Join-Path $scenarioDirectory "summary.json"
    Assert-Condition -Condition (Test-Path -LiteralPath $summaryPath) `
        -Message "Missing summary for scenario ${Name}: $summaryPath"
    $summary = Get-Content -LiteralPath $summaryPath -Raw | ConvertFrom-Json

    Assert-Condition -Condition ($exitCode -eq $ExpectedExitCode) `
        -Message "Scenario ${Name} exit code ${exitCode} != ${ExpectedExitCode}"
    Assert-Condition -Condition ($summary.result -ne "failed" -or $ExpectedExitCode -ne 0) `
        -Message "Scenario ${Name} unexpectedly failed"

    return [pscustomobject]@{
        name = $Name
        expected_exit_code = $ExpectedExitCode
        exit_code = $exitCode
        child_process_identity = $childIdentity
        output_path = $outputPath
        summary_path = $summaryPath
        summary = $summary
    }
}

function Invoke-ExpectedTimeoutScenario {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
        [Parameter(Mandatory = $true)][string[]]$AdditionalArguments
    )
    $scenarioDirectory = Join-Path $evidence $Name
    New-Item -ItemType Directory -Force -Path $scenarioDirectory | Out-Null
    $relativeEvidence = $scenarioDirectory.Substring($repoRoot.Length).TrimStart("\", "/")
    $outputPath = Join-Path $scenarioDirectory "console.log"
    $stdoutPath = Join-Path $scenarioDirectory "stdout.log"
    $stderrPath = Join-Path $scenarioDirectory "stderr.log"
    foreach ($path in @($outputPath, $stdoutPath, $stderrPath)) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force
        }
    }
    $arguments = @(
        "-NoProfile",
        "-File",
        $probePath,
        "-EvidenceRoot",
        $relativeEvidence,
        "-CleanupTimeoutSeconds",
        [string]$CleanupTimeoutSeconds
    ) + $AdditionalArguments
    $child = Start-Process `
        -FilePath $pwshPath `
        -ArgumentList $arguments `
        -WindowStyle Hidden `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath `
        -PassThru
    $childIdentity = Get-ProcessIdentity -ProcessId ([int]$child.Id)
    $timedOut = -not $child.WaitForExit($TimeoutSeconds * 1000)
    if (-not $timedOut) {
        throw "Watchdog scenario ${Name} exited before the expected timeout"
    }

    $ownershipPath = Join-Path $scenarioDirectory "ownership.json"
    Assert-Condition -Condition (Test-Path -LiteralPath $ownershipPath) `
        -Message "Watchdog scenario ${Name} did not write an ownership journal"
    $ownership = Get-Content -LiteralPath $ownershipPath -Raw -Encoding utf8 |
        ConvertFrom-Json
    $ownedIdentity = $ownership.process_identity
    Assert-Condition -Condition (
        $ownedIdentity.process_name -eq "EXCEL" -and
        (Test-ProcessIdentity -Identity $ownedIdentity)
    ) -Message "Watchdog scenario ${Name} ownership identity was not live before cleanup"

    $excelCleanup = Invoke-VerifiedProcessTreeTermination -Identity $ownedIdentity
    $childCleanup = Invoke-VerifiedProcessTreeTermination -Identity $childIdentity
    if (-not $child.HasExited) {
        [void]$child.WaitForExit(10000)
    }
    $remaining = Wait-ProcessIdentitiesGone `
        -Identities @($ownedIdentity, $childIdentity) `
        -TimeoutSeconds $CleanupTimeoutSeconds
    Assert-Condition -Condition ($remaining.Count -eq 0) `
        -Message "Watchdog scenario ${Name} left owned processes"
    Assert-Condition -Condition ($null -eq $excelCleanup.error) `
        -Message "Watchdog scenario ${Name} Excel cleanup failed: $($excelCleanup.error)"
    Assert-Condition -Condition ($null -eq $childCleanup.error) `
        -Message "Watchdog scenario ${Name} child cleanup failed: $($childCleanup.error)"

    $stdout = if (Test-Path -LiteralPath $stdoutPath) {
        Get-Content -LiteralPath $stdoutPath -Raw -Encoding utf8
    } else {
        ""
    }
    $stderr = if (Test-Path -LiteralPath $stderrPath) {
        Get-Content -LiteralPath $stderrPath -Raw -Encoding utf8
    } else {
        ""
    }
    $normalizedStdout = "$stdout".TrimEnd()
    $normalizedStderr = "$stderr".TrimEnd()
    $consoleText = @(
        "expected_timeout_seconds=$TimeoutSeconds",
        "stdout:",
        $normalizedStdout,
        "stderr:",
        $normalizedStderr
    ) -join "`n"
    Write-Utf8File -Path $outputPath -Text ($consoleText.TrimEnd() + "`n")
    return [pscustomobject]@{
        name = $Name
        expected_timeout_seconds = $TimeoutSeconds
        timed_out = $true
        owned_process_identity = $ownedIdentity
        child_process_identity = $childIdentity
        excel_cleanup = $excelCleanup
        child_cleanup = $childCleanup
        remaining_process_identities = @($remaining)
        output_path = $outputPath
    }
}

function Cleanup-BootstrapExcel {
    param(
        [AllowNull()]$Excel,
        [AllowNull()]$Workbook,
        [AllowNull()]$Worksheet,
        [AllowNull()]$Range,
        [AllowEmptyCollection()][object[]]$OwnedIdentities
    )
    $cleanupFailures = New-Object System.Collections.Generic.List[string]
    $taskkillExitCodes = New-Object System.Collections.Generic.List[string]

    if ($null -ne $Workbook) {
        try {
            $Workbook.Close($false)
        } catch {
            $cleanupFailures.Add("Bootstrap workbook close failed: $($_.Exception.Message)")
        }
    }
    if ($null -ne $Excel) {
        try {
            $Excel.Quit()
        } catch {
            $cleanupFailures.Add("Bootstrap Excel Quit failed: $($_.Exception.Message)")
        }
    }
    foreach ($release in @(
        @{ Object = $Range; Name = "Bootstrap Range" },
        @{ Object = $Worksheet; Name = "Bootstrap Worksheet" },
        @{ Object = $Workbook; Name = "Bootstrap Workbook" },
        @{ Object = $Excel; Name = "Bootstrap Excel Application" }
    )) {
        $releaseFailure = Release-ComObject -ComObject $release.Object -Name $release.Name
        if ($null -ne $releaseFailure) {
            $cleanupFailures.Add($releaseFailure)
        }
    }

    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()

    $remaining = Wait-ProcessIdentitiesGone `
        -Identities $OwnedIdentities `
        -TimeoutSeconds $CleanupTimeoutSeconds
    foreach ($remainingIdentity in $remaining) {
        $taskkillResult = Invoke-VerifiedProcessTreeTermination -Identity $remainingIdentity
        if ($null -ne $taskkillResult.exit_code) {
            $taskkillExitCodes.Add([string]$taskkillResult.exit_code)
        }
        if ($null -ne $taskkillResult.error) {
            $cleanupFailures.Add(
                "Bootstrap taskkill failed for PID $($remainingIdentity.id): " +
                "$($taskkillResult.error)"
            )
        }
    }
    if ($remaining.Count -ne 0) {
        $remaining = Wait-ProcessIdentitiesGone `
            -Identities $remaining `
            -TimeoutSeconds $CleanupTimeoutSeconds
    }

    return [pscustomobject]@{
        cleanup_failures = @($cleanupFailures)
        taskkill_exit_codes = @($taskkillExitCodes)
        remaining_process_identities = @($remaining)
    }
}

function Start-BootstrapExcelHelper {
    param([Parameter(Mandatory = $true)][string]$ScenarioDirectory)
    $ownershipPath = Join-Path $ScenarioDirectory "bootstrap-ownership.json"
    $readyPath = Join-Path $ScenarioDirectory "bootstrap-ready.signal"
    $stopPath = Join-Path $ScenarioDirectory "bootstrap-stop.signal"
    $stdoutPath = Join-Path $ScenarioDirectory "bootstrap-stdout.log"
    $stderrPath = Join-Path $ScenarioDirectory "bootstrap-stderr.log"
    foreach ($path in @($ownershipPath, $readyPath, $stopPath, $stdoutPath, $stderrPath)) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force
        }
    }
    $relativeEvidence = $ScenarioDirectory.Substring($repoRoot.Length).TrimStart("\", "/")
    $arguments = @(
        "-NoProfile",
        "-File",
        $PSCommandPath,
        "-EvidenceRoot",
        $relativeEvidence,
        "-CleanupTimeoutSeconds",
        [string]$CleanupTimeoutSeconds,
        "-ProbeTimeoutSeconds",
        [string]$ProbeTimeoutSeconds,
        "-BootstrapTimeoutSeconds",
        [string]$BootstrapTimeoutSeconds,
        "-BootstrapHelper",
        "-BootstrapOwnershipPath",
        $ownershipPath,
        "-BootstrapReadySignalPath",
        $readyPath,
        "-BootstrapStopSignalPath",
        $stopPath
    )
    $helper = $null
    $helperIdentity = $null
    $excelIdentity = $null
    try {
        $helper = Start-Process `
            -FilePath $pwshPath `
            -ArgumentList $arguments `
            -WindowStyle Hidden `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath `
            -PassThru
        $helperIdentity = Get-ProcessIdentity -ProcessId ([int]$helper.Id)
        $deadline = (Get-Date).ToUniversalTime().AddSeconds($BootstrapTimeoutSeconds)
        while (-not (Test-Path -LiteralPath $readyPath)) {
            if ($helper.HasExited) {
                throw "Bootstrap helper exited before becoming ready"
            }
            if ((Get-Date).ToUniversalTime() -ge $deadline) {
                throw "Bootstrap helper did not become ready before its deadline"
            }
            Start-Sleep -Milliseconds 200
        }
        $ownership = Get-Content -LiteralPath $ownershipPath -Raw -Encoding utf8 |
            ConvertFrom-Json
        $excelIdentity = $ownership.process_identity
        $identityValid = (
            $null -ne $excelIdentity -and
            $excelIdentity.process_name -eq "EXCEL" -and
            (Test-ProcessIdentity -Identity $excelIdentity)
        )
        if (-not $identityValid) {
            throw "Bootstrap helper ownership was missing or invalid"
        }
        return [pscustomobject]@{
            process = $helper
            process_identity = $helperIdentity
            excel_identity = $excelIdentity
            ownership_path = $ownershipPath
            ready_path = $readyPath
            stop_path = $stopPath
            stdout_path = $stdoutPath
            stderr_path = $stderrPath
        }
    } catch {
        if ($null -ne $excelIdentity) {
            [void](Invoke-VerifiedProcessTreeTermination -Identity $excelIdentity)
        } elseif (Test-Path -LiteralPath $ownershipPath) {
            $ownership = Get-Content -LiteralPath $ownershipPath -Raw -Encoding utf8 |
                ConvertFrom-Json
            if ($null -ne $ownership.process_identity) {
                [void](Invoke-VerifiedProcessTreeTermination `
                    -Identity $ownership.process_identity)
            }
        }
        if ($null -ne $helperIdentity) {
            [void](Invoke-VerifiedProcessTreeTermination -Identity $helperIdentity)
        }
        $helperError = if (Test-Path -LiteralPath $stderrPath) {
            Get-Content -LiteralPath $stderrPath -Raw -Encoding utf8
        } else {
            ""
        }
        throw "Bootstrap helper startup failed: $($_.Exception.Message) stderr=$helperError"
    }
}

if ($BootstrapHelper) {
    if (
        [string]::IsNullOrWhiteSpace($BootstrapOwnershipPath) -or
        [string]::IsNullOrWhiteSpace($BootstrapReadySignalPath) -or
        [string]::IsNullOrWhiteSpace($BootstrapStopSignalPath)
    ) {
        throw "Bootstrap helper requires ownership, ready, and stop signal paths"
    }
    $helperFailure = $null
    $excel = $null
    $workbook = $null
    $worksheet = $null
    $range = $null
    $ownedIdentity = $null
    $helperCleanup = $null
    try {
        $existingBeforeHelper = Get-ExcelProcesses
        Assert-Condition -Condition ($existingBeforeHelper.Count -eq 0) `
            -Message "Bootstrap helper requires zero pre-existing EXCEL.EXE processes"

        $excel = New-Object -ComObject Excel.Application
        $ownedProcessId = Get-ExcelApplicationProcessId -Excel $excel
        $ownedIdentity = Get-ProcessIdentity -ProcessId $ownedProcessId
        Assert-Condition -Condition (
            $null -ne $ownedIdentity -and $ownedIdentity.process_name -eq "EXCEL"
        ) -Message "Bootstrap helper could not establish process ownership"
        Write-JsonFile -Path $BootstrapOwnershipPath -Value ([pscustomobject]@{
            schema_version = 1
            state = "owned"
            established_at_utc = (Get-Date).ToUniversalTime().ToString("o")
            process_identity = $ownedIdentity
        })

        $excel.Visible = $false
        $workbook = $excel.Workbooks.Add()
        $worksheet = $workbook.Worksheets.Item(1)
        $range = $worksheet.Range("A1")
        $range.Value2 = "lifecycle refusal sentinel"
        Write-Utf8File -Path $BootstrapReadySignalPath -Text "ready`n"

        $deadline = (Get-Date).ToUniversalTime().AddSeconds($BootstrapTimeoutSeconds)
        while (-not (Test-Path -LiteralPath $BootstrapStopSignalPath)) {
            if ((Get-Date).ToUniversalTime() -ge $deadline) {
                throw "Bootstrap helper did not receive a stop signal before its deadline"
            }
            Start-Sleep -Milliseconds 200
        }
    } catch {
        $helperFailure = $_.Exception.Message
    } finally {
        $helperCleanup = Cleanup-BootstrapExcel `
            -Excel $excel `
            -Workbook $workbook `
            -Worksheet $worksheet `
            -Range $range `
            -OwnedIdentities @($ownedIdentity)
        Write-JsonFile -Path $BootstrapOwnershipPath -Value ([pscustomobject]@{
            schema_version = 1
            state = if (
                $helperCleanup.remaining_process_identities.Count -eq 0
            ) {
                "reaped"
            } else {
                "cleanup_failed"
            }
            process_identity = $ownedIdentity
            cleanup = $helperCleanup
            completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        })
    }
    if ($helperFailure -ne $null) {
        [Console]::Error.WriteLine($helperFailure)
        exit 2
    }
    exit 0
}

$overallFailure = $null
$scenarioEvidence = [ordered]@{}
$bootstrapCleanup = $null
$bootstrapHolder = $null
$bootstrapOwnedIdentities = @()
$bootstrapRefusalProcessAlive = $null

try {
    $existingAtStart = Get-ExcelProcesses
    Assert-Condition -Condition ($existingAtStart.Count -eq 0) `
        -Message "Lifecycle regression requires zero pre-existing EXCEL.EXE processes"

    $normal = Invoke-ProbeScenario -Name "normal" -ExpectedExitCode 0
    Assert-Condition -Condition ($normal.summary.result -eq "passed") `
        -Message "Normal scenario did not report passed"
    Assert-Condition -Condition ($normal.summary.failure_kind -eq "none") `
        -Message "Normal scenario reported a failure kind"
    Assert-Condition -Condition ($normal.summary.cleanup.cleanup_succeeded -eq $true) `
        -Message "Normal scenario cleanup did not succeed"
    Assert-Condition -Condition ($normal.summary.cleanup.all_probe_processes_reaped -eq $true) `
        -Message "Normal scenario left an owned process"
    Assert-Condition -Condition ($normal.summary.cleanup.excel_process_count_after -eq 0) `
        -Message "Normal scenario left Excel running"
    $scenarioEvidence.normal = $normal

    $injected = Invoke-ProbeScenario `
        -Name "injected-failure" `
        -ExpectedExitCode 2 `
        -AdditionalArguments @("-FailureInjectionPoint", "after-workbook-create")
    Assert-Condition -Condition ($injected.summary.result -eq "failed") `
        -Message "Injected failure scenario did not report failed"
    Assert-Condition -Condition ($injected.summary.failure_kind -eq "injected_failure") `
        -Message "Injected failure scenario reported the wrong failure kind"
    Assert-Condition -Condition (
        $injected.summary.failure_injection_point -eq "after-workbook-create"
    ) -Message "Injected failure scenario did not record its checkpoint"
    Assert-Condition -Condition ($injected.summary.cleanup.cleanup_succeeded -eq $true) `
        -Message "Injected failure scenario cleanup did not succeed"
    Assert-Condition -Condition (
        $injected.summary.cleanup.excel_process_count_after -eq 0
    ) -Message "Injected failure scenario left Excel running"
    $scenarioEvidence.injected_failure = $injected

    $cancelDirectory = Join-Path $evidence "cooperative-cancel"
    New-Item -ItemType Directory -Force -Path $cancelDirectory | Out-Null
    $cancelSignal = Join-Path $cancelDirectory "request.signal"
    Write-Utf8File -Path $cancelSignal -Text "cancel`n"
    $cancelled = Invoke-ProbeScenario `
        -Name "cooperative-cancel" `
        -ExpectedExitCode 3 `
        -AdditionalArguments @(
            "-CancellationSignalPath",
            $cancelSignal,
            "-CancellationCheckpoint",
            "after-workbook-create"
        )
    Assert-Condition -Condition ($cancelled.summary.result -eq "cancelled") `
        -Message "Cooperative cancellation scenario did not report cancelled"
    Assert-Condition -Condition ($cancelled.summary.failure_kind -eq "cancelled") `
        -Message "Cooperative cancellation scenario reported the wrong failure kind"
    Assert-Condition -Condition (
        $cancelled.summary.cancellation_signal_observed -eq $true
    ) -Message "Cooperative cancellation signal was not observed"
    Assert-Condition -Condition (
        $cancelled.summary.cancellation_checkpoint -eq "after-workbook-create"
    ) -Message "Cooperative cancellation recorded the wrong checkpoint"
    Assert-Condition -Condition ($cancelled.summary.cleanup.cleanup_succeeded -eq $true) `
        -Message "Cooperative cancellation scenario cleanup did not succeed"
    Assert-Condition -Condition (
        $cancelled.summary.cleanup.excel_process_count_after -eq 0
    ) -Message "Cooperative cancellation scenario left Excel running"
    $scenarioEvidence.cooperative_cancel = $cancelled

    $watchdog = Invoke-ExpectedTimeoutScenario `
        -Name "watchdog-timeout-cleanup" `
        -TimeoutSeconds 15 `
        -AdditionalArguments @(
            "-HangAfterCheckpoint",
            "after-workbook-create"
        )
    $scenarioEvidence.watchdog_timeout_cleanup = $watchdog

    $bootstrapScenarioDirectory = Join-Path $evidence "existing-process-refusal"
    New-Item -ItemType Directory -Force -Path $bootstrapScenarioDirectory | Out-Null
    $bootstrapHolder = Start-BootstrapExcelHelper `
        -ScenarioDirectory $bootstrapScenarioDirectory
    $bootstrapOwnedIdentities = @($bootstrapHolder.excel_identity)

    $refusal = Invoke-ProbeScenario -Name "existing-process-refusal" -ExpectedExitCode 2
    Assert-Condition -Condition ($refusal.summary.result -eq "failed") `
        -Message "Existing-process refusal scenario did not report failed"
    Assert-Condition -Condition ($refusal.summary.failure_kind -eq "existing_process") `
        -Message "Existing-process refusal reported the wrong failure kind"
    Assert-Condition -Condition (
        @($refusal.summary.existing_excel_process_ids) -contains
            ([int]$bootstrapHolder.excel_identity.id)
    ) -Message "Existing-process refusal did not identify the bootstrap Excel"
    $bootstrapRefusalProcessAlive = Test-ProcessIdentity `
        -Identity $bootstrapHolder.excel_identity
    Assert-Condition -Condition $bootstrapRefusalProcessAlive `
        -Message "Probe terminated a pre-existing EXCEL.EXE process"
    Assert-Condition -Condition (
        $refusal.summary.cleanup.probe_created_any_process -eq $false
    ) -Message "Existing-process refusal claimed to create a probe process"
    Assert-Condition -Condition (
        @($refusal.summary.existing_excel_process_identities_still_alive).Count -ge 1
    ) -Message "Refusal summary did not record the surviving bootstrap identity"
    $scenarioEvidence.existing_process_refusal = $refusal

    Write-Utf8File -Path $bootstrapHolder.stop_path -Text "stop`n"
    $helperCompleted = $bootstrapHolder.process.WaitForExit(
        $BootstrapTimeoutSeconds * 1000
    )
    $helperTaskkill = $null
    $excelTaskkill = $null
    if (-not $helperCompleted) {
        $helperTaskkill = Invoke-VerifiedProcessTreeTermination `
            -Identity $bootstrapHolder.process_identity
        $excelTaskkill = Invoke-VerifiedProcessTreeTermination `
            -Identity $bootstrapHolder.excel_identity
    } else {
        $bootstrapHolder.process.WaitForExit()
        Assert-Condition -Condition ($bootstrapHolder.process.ExitCode -eq 0) `
            -Message "Bootstrap helper cleanup exited with $($bootstrapHolder.process.ExitCode)"
    }
    $bootstrapRemaining = Wait-ProcessIdentitiesGone `
        -Identities $bootstrapOwnedIdentities `
        -TimeoutSeconds $CleanupTimeoutSeconds
    if ($bootstrapRemaining.Count -ne 0) {
        $excelTaskkill = Invoke-VerifiedProcessTreeTermination `
            -Identity $bootstrapHolder.excel_identity
        $bootstrapRemaining = Wait-ProcessIdentitiesGone `
            -Identities $bootstrapOwnedIdentities `
            -TimeoutSeconds $CleanupTimeoutSeconds
    }
    $bootstrapCleanup = [pscustomobject]@{
        mode = if ($helperCompleted) { "helper-self-cleanup" } else { "helper-timeout-fallback" }
        helper_exit_code = if ($helperCompleted) {
            $bootstrapHolder.process.ExitCode
        } else {
            $null
        }
        helper_taskkill = $helperTaskkill
        excel_taskkill = $excelTaskkill
        remaining_process_identities = @($bootstrapRemaining)
    }
    Assert-Condition -Condition ($bootstrapRemaining.Count -eq 0) `
        -Message "Bootstrap cleanup left its owned Excel process"
} catch {
    $overallFailure = "$($_.Exception.Message)`n$($_.ScriptStackTrace)"
} finally {
    if ($null -ne $bootstrapHolder) {
        if (-not $bootstrapHolder.process.HasExited) {
            [void](Invoke-VerifiedProcessTreeTermination `
                -Identity $bootstrapHolder.process_identity)
        }
        if (Test-ProcessIdentity -Identity $bootstrapHolder.excel_identity) {
            [void](Invoke-VerifiedProcessTreeTermination `
                -Identity $bootstrapHolder.excel_identity)
        }
    }
}

[void](Wait-ExcelProcessCountZero -TimeoutSeconds $CleanupTimeoutSeconds)
$excelProcessesAfter = Get-ExcelProcesses
$excelProcessIdsAfter = Get-ProcessIds $excelProcessesAfter
if ($overallFailure -eq $null -and $excelProcessesAfter.Count -ne 0) {
    $overallFailure = "Lifecycle regression left EXCEL.EXE processes: $($excelProcessIdsAfter -join ',')"
}

$completedAt = (Get-Date).ToUniversalTime().ToString("o")
$summaryPath = Join-Path $evidence "summary.json"
$summary = [pscustomobject]@{
    schema_version = 1
    mode = "excel-com-lifecycle-regression"
    result = if ($overallFailure -eq $null) { "passed" } else { "failed" }
    failure = $overallFailure
    scenarios = [pscustomobject]@{
        normal = $scenarioEvidence.normal
        injected_failure = $scenarioEvidence.injected_failure
        cooperative_cancel = $scenarioEvidence.cooperative_cancel
        watchdog_timeout_cleanup = $scenarioEvidence.watchdog_timeout_cleanup
        existing_process_refusal = $scenarioEvidence.existing_process_refusal
    }
    bootstrap_process_identity = if ($null -eq $bootstrapHolder) {
        $null
    } else {
        $bootstrapHolder.excel_identity
    }
    bootstrap_process_alive_after_refusal = $bootstrapRefusalProcessAlive
    bootstrap_cleanup = $bootstrapCleanup
    final_excel_process_ids = $excelProcessIdsAfter
    final_excel_process_count = $excelProcessesAfter.Count
    started_at_utc = $startedAt
    completed_at_utc = $completedAt
}
Write-JsonFile -Path $summaryPath -Value $summary

if ($overallFailure -ne $null) {
    [Console]::Error.WriteLine($overallFailure)
    exit 2
}

Write-Output (
    "excel-com-lifecycle passed scenarios=5 " +
    "final_excel_process_count=$($excelProcessesAfter.Count)"
)
exit 0
