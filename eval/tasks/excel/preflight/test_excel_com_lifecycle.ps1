param(
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [ValidateRange(5, 120)]
    [int]$CleanupTimeoutSeconds = 30
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

function Get-ProcessIds {
    param([object[]]$Processes)
    return @($Processes | ForEach-Object { [int]$_.Id })
}

function Wait-ProcessIdsGone {
    param(
        [int[]]$ProcessIds,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )
    for ($attempt = 0; $attempt -lt ($TimeoutSeconds * 4); $attempt++) {
        $remaining = @(
            $ProcessIds |
                Where-Object {
                    $null -ne (Get-Process -Id $_ -ErrorAction SilentlyContinue)
                }
        )
        if ($remaining.Count -eq 0) {
            return @()
        }
        Start-Sleep -Milliseconds 250
    }
    return @(
        $ProcessIds |
            Where-Object {
                $null -ne (Get-Process -Id $_ -ErrorAction SilentlyContinue)
            }
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

    $output = (& $pwshPath @arguments 2>&1 | Out-String)
    $exitCode = [int]$LASTEXITCODE
    $outputPath = Join-Path $scenarioDirectory "console.log"
    Write-Utf8File -Path $outputPath -Text $output

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
        output_path = $outputPath
        summary_path = $summaryPath
        summary = $summary
    }
}

function Cleanup-BootstrapExcel {
    param(
        [AllowNull()]$Excel,
        [AllowNull()]$Workbook,
        [AllowNull()]$Worksheet,
        [AllowNull()]$Range,
        [int[]]$OwnedProcessIds
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

    $remaining = Wait-ProcessIdsGone `
        -ProcessIds $OwnedProcessIds `
        -TimeoutSeconds $CleanupTimeoutSeconds
    foreach ($processId in $remaining) {
        try {
            $taskkill = Start-Process `
                -FilePath "taskkill.exe" `
                -ArgumentList @("/PID", "$processId", "/T", "/F") `
                -WindowStyle Hidden `
                -Wait `
                -PassThru
            $taskkillExitCodes.Add([string]$taskkill.ExitCode)
        } catch {
            $cleanupFailures.Add(
                "Bootstrap taskkill spawn failed for PID ${processId}: $($_.Exception.Message)"
            )
        }
    }
    if ($remaining.Count -ne 0) {
        $remaining = Wait-ProcessIdsGone `
            -ProcessIds $remaining `
            -TimeoutSeconds $CleanupTimeoutSeconds
    }

    return [pscustomobject]@{
        cleanup_failures = @($cleanupFailures)
        taskkill_exit_codes = @($taskkillExitCodes)
        remaining_process_ids = @($remaining)
    }
}

$overallFailure = $null
$scenarioEvidence = [ordered]@{}
$bootstrapCleanup = $null
$bootstrapExcel = $null
$bootstrapWorkbook = $null
$bootstrapWorksheet = $null
$bootstrapRange = $null
$bootstrapProcessIds = @()

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

    $bootstrapBefore = Get-ExcelProcesses
    $bootstrapExcel = New-Object -ComObject Excel.Application
    $bootstrapExcel.Visible = $false
    $bootstrapWorkbook = $bootstrapExcel.Workbooks.Add()
    $bootstrapWorksheet = $bootstrapWorkbook.Worksheets.Item(1)
    $bootstrapRange = $bootstrapWorksheet.Range("A1")
    $bootstrapRange.Value2 = "lifecycle refusal sentinel"
    $bootstrapAfterStart = Get-ExcelProcesses
    $bootstrapProcessIds = @(
        $bootstrapAfterStart |
            Where-Object { $_.Id -notin (Get-ProcessIds $bootstrapBefore) } |
            ForEach-Object { [int]$_.Id }
    )
    Assert-Condition -Condition ($bootstrapProcessIds.Count -ge 1) `
        -Message "Bootstrap Excel did not expose a new EXCEL.EXE process id"

    $refusal = Invoke-ProbeScenario -Name "existing-process-refusal" -ExpectedExitCode 2
    Assert-Condition -Condition ($refusal.summary.result -eq "failed") `
        -Message "Existing-process refusal scenario did not report failed"
    Assert-Condition -Condition ($refusal.summary.failure_kind -eq "existing_process") `
        -Message "Existing-process refusal reported the wrong failure kind"
    Assert-Condition -Condition (
        @($refusal.summary.existing_excel_process_ids) -contains $bootstrapProcessIds[0]
    ) -Message "Existing-process refusal did not identify the bootstrap Excel"
    foreach ($bootstrapProcessId in $bootstrapProcessIds) {
        Assert-Condition -Condition (
            $null -ne (Get-Process -Id $bootstrapProcessId -ErrorAction SilentlyContinue)
        ) -Message "Probe terminated a pre-existing EXCEL.EXE process"
    }
    Assert-Condition -Condition (
        $refusal.summary.cleanup.probe_created_any_process -eq $false
    ) -Message "Existing-process refusal claimed to create a probe process"
    $scenarioEvidence.existing_process_refusal = $refusal

    $bootstrapCleanup = Cleanup-BootstrapExcel `
        -Excel $bootstrapExcel `
        -Workbook $bootstrapWorkbook `
        -Worksheet $bootstrapWorksheet `
        -Range $bootstrapRange `
        -OwnedProcessIds $bootstrapProcessIds
    $bootstrapExcel = $null
    $bootstrapWorkbook = $null
    $bootstrapWorksheet = $null
    $bootstrapRange = $null
    Assert-Condition -Condition ($bootstrapCleanup.cleanup_failures.Count -eq 0) `
        -Message "Bootstrap cleanup recorded failures"
    Assert-Condition -Condition ($bootstrapCleanup.remaining_process_ids.Count -eq 0) `
        -Message "Bootstrap cleanup left owned processes"
} catch {
    $overallFailure = $_.Exception.Message
} finally {
    if ($null -ne $bootstrapExcel -or $null -ne $bootstrapWorkbook) {
        $bootstrapCleanup = Cleanup-BootstrapExcel `
            -Excel $bootstrapExcel `
            -Workbook $bootstrapWorkbook `
            -Worksheet $bootstrapWorksheet `
            -Range $bootstrapRange `
            -OwnedProcessIds $bootstrapProcessIds
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
        existing_process_refusal = $scenarioEvidence.existing_process_refusal
    }
    bootstrap_cleanup = $bootstrapCleanup
    final_excel_process_ids = $excelProcessIdsAfter
    final_excel_process_count = $excelProcessesAfter.Count
    started_at_utc = $startedAt
    completed_at_utc = $completedAt
}
Write-JsonFile -Path $summaryPath -Value $summary

if ($overallFailure -ne $null) {
    Write-Error $overallFailure
    exit 2
}

Write-Output (
    "excel-com-lifecycle passed scenarios=4 " +
    "final_excel_process_count=$($excelProcessesAfter.Count)"
)
exit 0
