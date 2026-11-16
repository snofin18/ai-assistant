param(
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [ValidateRange(5, 120)]
    [int]$CleanupTimeoutSeconds = 30,

    [ValidateSet("none", "after-com-create", "after-workbook-create", "after-formula")]
    [string]$FailureInjectionPoint = "none",

    [AllowNull()]
    [string]$CancellationSignalPath = $null,

    [ValidateSet("after-com-create", "after-workbook-create", "after-formula")]
    [string]$CancellationCheckpoint = "after-workbook-create"
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$summaryPath = Join-Path $evidence "summary.json"
$cancellationSignal = if ([string]::IsNullOrWhiteSpace($CancellationSignalPath)) {
    $null
} else {
    [System.IO.Path]::GetFullPath($CancellationSignalPath)
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    $text = (($Value | ConvertTo-Json -Depth 12) + "`n") -replace "`r`n", "`n"
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $text, $encoding)
}

function Get-ExcelProcesses {
    return @(Get-Process EXCEL -ErrorAction SilentlyContinue)
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

function Invoke-CooperativeCheckpoint {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet("after-com-create", "after-workbook-create", "after-formula")]
        [string]$Checkpoint
    )
    if (
        $Checkpoint -eq $CancellationCheckpoint -and
        $null -ne $cancellationSignal -and
        (Test-Path -LiteralPath $cancellationSignal)
    ) {
        throw [System.OperationCanceledException]::new(
            "Cooperative cancellation requested at checkpoint ${Checkpoint}"
        )
    }
    if ($FailureInjectionPoint -eq $Checkpoint) {
        throw "Injected failure at checkpoint ${Checkpoint}"
    }
}

function Wait-NewExcelProcessesGone {
    param(
        [Parameter(Mandatory = $true)][int[]]$ProcessIds,
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

$startedAt = (Get-Date).ToUniversalTime().ToString("o")
$existingProcesses = Get-ExcelProcesses
$existingProcessIds = @($existingProcesses | ForEach-Object { [int]$_.Id })
if ($existingProcesses.Count -ne 0) {
    $summary = [pscustomobject]@{
        schema_version = 2
        mode = "excel-com-preflight"
        result = "failed"
        failure_kind = "existing_process"
        failure = "Existing EXCEL.EXE process detected; refusing to reuse or terminate it"
        failure_injection_point = $FailureInjectionPoint
        cancellation_signal_path = $cancellationSignal
        cancellation_checkpoint = $CancellationCheckpoint
        existing_excel_process_ids = $existingProcessIds
        cleanup = [pscustomobject]@{
            cleanup_succeeded = $true
            probe_created_any_process = $false
            all_probe_processes_reaped = $true
            excel_process_count_after = $existingProcesses.Count
        }
        started_at_utc = $startedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    }
    Write-JsonFile -Path $summaryPath -Value $summary
    [Console]::Error.WriteLine([string]$summary.failure)
    exit 2
}

$excel = $null
$workbook = $null
$worksheet = $null
$rangeA1 = $null
$rangeA2 = $null
$rangeA3 = $null
$failure = $null
$failureKind = "none"
$cancellationObserved = $false
$cleanupFailures = New-Object System.Collections.Generic.List[string]
$taskkillExitCodes = New-Object System.Collections.Generic.List[string]
$newProcessIds = @()
$remainingProcessIds = @()
$workbookCloseAttempted = $false
$workbookClosedWithoutSave = $false
$quitAttempted = $false
$quitSucceeded = $false
$excelVersion = $null
$excelBuild = $null
$excelPath = $null
$protectedViewWindowCount = $null
$autoSaveOn = $null
$displayAlerts = $null
$visible = $null
$canUndoBefore = $null
$canUndoAfterFirstWrite = $null
$canUndoAfterSecondWrite = $null
$undoInvoked = $false
$valueAfterUndo = $null
$undoObservationError = $null
$formulaActual = $null
$formulaExpression = $null
$formulaText = $null

try {
    $excel = New-Object -ComObject Excel.Application
    $processesAfterStart = Get-ExcelProcesses
    $newProcessIds = @(
        $processesAfterStart |
            Where-Object { $_.Id -notin $existingProcessIds } |
            ForEach-Object { [int]$_.Id }
    )
    if ($newProcessIds.Count -eq 0) {
        throw "Excel COM started without a discoverable EXCEL.EXE process"
    }
    Invoke-CooperativeCheckpoint -Checkpoint "after-com-create"

    $excelVersion = [string]$excel.Version
    $excelBuild = [string]$excel.Build
    $excelPath = [string]$excel.Path
    $displayAlerts = [int]$excel.DisplayAlerts
    $visible = [bool]$excel.Visible
    $protectedViewWindowCount = [int]$excel.ProtectedViewWindows.Count

    $workbook = $excel.Workbooks.Add()
    try {
        $autoSaveOn = [bool]$workbook.AutoSaveOn
    } catch {
        $autoSaveOn = $null
    }
    Invoke-CooperativeCheckpoint -Checkpoint "after-workbook-create"
    $worksheet = $workbook.Worksheets.Item(1)
    $rangeA1 = $worksheet.Range("A1")
    $rangeA2 = $worksheet.Range("A2")
    $rangeA3 = $worksheet.Range("A3")

    $canUndoBefore = [bool]$excel.CanUndo
    $rangeA1.Value2 = 1
    $canUndoAfterFirstWrite = [bool]$excel.CanUndo
    $rangeA2.Value2 = 2
    $canUndoAfterSecondWrite = [bool]$excel.CanUndo

    try {
        if ([bool]$excel.CanUndo) {
            $excel.Undo()
            $undoInvoked = $true
            $valueAfterUndo = $rangeA2.Value2
        }
    } catch {
        $undoObservationError = $_.Exception.Message
    }

    $rangeA1.Value2 = 2
    $rangeA2.Value2 = 3
    $rangeA3.Formula = "=A1+A2"
    $excel.Calculate()
    $formulaActual = [double]$rangeA3.Value2
    $formulaExpression = [string]$rangeA3.Formula
    $formulaText = [string]$rangeA3.Text
    Invoke-CooperativeCheckpoint -Checkpoint "after-formula"
    if ($formulaActual -ne 5) {
        throw "Formula preflight expected 5 but observed $formulaActual"
    }
} catch [System.OperationCanceledException] {
    $failureKind = "cancelled"
    $cancellationObserved = $true
    $failure = $_.Exception.Message
} catch {
    $failureKind = if ($_.Exception.Message.StartsWith(
        "Injected failure at checkpoint ",
        [System.StringComparison]::Ordinal
    )) {
        "injected_failure"
    } else {
        "exception"
    }
    $failure = $_.Exception.Message
} finally {
    if ($null -ne $workbook) {
        $workbookCloseAttempted = $true
        try {
            $workbook.Close($false)
            $workbookClosedWithoutSave = $true
        } catch {
            $cleanupFailures.Add("Workbook close failed: $($_.Exception.Message)")
        }
    }
    if ($null -ne $excel) {
        $quitAttempted = $true
        try {
            $excel.Quit()
            $quitSucceeded = $true
        } catch {
            $cleanupFailures.Add("Excel Quit failed: $($_.Exception.Message)")
        }
    }
    foreach ($release in @(
        @{ Object = $rangeA3; Name = "Range A3" },
        @{ Object = $rangeA2; Name = "Range A2" },
        @{ Object = $rangeA1; Name = "Range A1" },
        @{ Object = $worksheet; Name = "Worksheet" },
        @{ Object = $workbook; Name = "Workbook" },
        @{ Object = $excel; Name = "Excel Application" }
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

    $remainingProcessIds = Wait-NewExcelProcessesGone `
        -ProcessIds $newProcessIds `
        -TimeoutSeconds $CleanupTimeoutSeconds
    foreach ($processId in $remainingProcessIds) {
        try {
            $taskkill = Start-Process `
                -FilePath "taskkill.exe" `
                -ArgumentList @("/PID", "$processId", "/T", "/F") `
                -WindowStyle Hidden `
                -Wait `
                -PassThru
            $taskkillExitCodes.Add([string]$taskkill.ExitCode)
        } catch {
            $cleanupFailures.Add("taskkill spawn failed for PID ${processId}: $($_.Exception.Message)")
        }
    }
    if ($remainingProcessIds.Count -ne 0) {
        $remainingProcessIds = Wait-NewExcelProcessesGone `
            -ProcessIds $newProcessIds `
            -TimeoutSeconds $CleanupTimeoutSeconds
    }
    if ($remainingProcessIds.Count -ne 0 -and $failure -eq $null) {
        $failureKind = "cleanup_failure"
        $failure = "Probe-created EXCEL.EXE processes did not converge"
    }
}

$excelProcessesAfter = Get-ExcelProcesses
$completedAt = (Get-Date).ToUniversalTime().ToString("o")
$allProbeProcessesReaped = (
    $newProcessIds.Count -eq 0 -or
    @(
        $newProcessIds |
            Where-Object {
                $null -ne (Get-Process -Id $_ -ErrorAction SilentlyContinue)
            }
    ).Count -eq 0
)
$cleanupSucceeded = (
    $cleanupFailures.Count -eq 0 -and
    $allProbeProcessesReaped -and
    $excelProcessesAfter.Count -eq 0
)
$result = if ($failureKind -eq "none" -and $failure -eq $null -and $cleanupSucceeded) {
    "passed"
} elseif ($failureKind -eq "cancelled" -and $cleanupSucceeded) {
    "cancelled"
} else {
    "failed"
}
$summary = [pscustomobject]@{
    schema_version = 2
    mode = "excel-com-preflight"
    result = $result
    failure_kind = $failureKind
    failure = if ($failure -eq $null) { $null } else { [string]$failure }
    failure_injection_point = $FailureInjectionPoint
    cancellation_signal_path = $cancellationSignal
    cancellation_checkpoint = $CancellationCheckpoint
    cancellation_signal_observed = $cancellationObserved
    excel_version = $excelVersion
    excel_build = $excelBuild
    excel_path = $excelPath
    probe_excel_process_ids = $newProcessIds
    display_alerts_observed = $displayAlerts
    display_alerts_changed = $false
    visible_observed = $visible
    protected_view_window_count = $protectedViewWindowCount
    autosave_on = $autoSaveOn
    undo_observation = [pscustomobject]@{
        can_undo_before = $canUndoBefore
        can_undo_after_first_write = $canUndoAfterFirstWrite
        can_undo_after_second_write = $canUndoAfterSecondWrite
        application_undo_invoked = $undoInvoked
        value_after_undo = $valueAfterUndo
        error = $undoObservationError
    }
    formula_observation = [pscustomobject]@{
        expected_value2 = 5
        actual_value2 = $formulaActual
        formula = $formulaExpression
        text = $formulaText
    }
    cleanup = [pscustomobject]@{
        workbook_close_attempted = $workbookCloseAttempted
        workbook_closed_without_save = $workbookClosedWithoutSave
        quit_attempted = $quitAttempted
        quit_succeeded = $quitSucceeded
        cleanup_failures = @($cleanupFailures)
        taskkill_exit_codes = @($taskkillExitCodes)
        remaining_probe_process_ids = @($remainingProcessIds)
        excel_process_count_after = $excelProcessesAfter.Count
        all_probe_processes_reaped = $allProbeProcessesReaped
        cleanup_succeeded = $cleanupSucceeded
    }
    started_at_utc = $startedAt
    completed_at_utc = $completedAt
}
Write-JsonFile -Path $summaryPath -Value $summary

if ($result -eq "cancelled") {
    [Console]::Error.WriteLine([string]$failure)
    exit 3
}

if ($result -ne "passed") {
    [Console]::Error.WriteLine([string]$failure)
    exit 2
}

Write-Output (
    "excel-com-preflight passed version=$excelVersion build=$excelBuild " +
    "formula_value2=$formulaActual excel_processes_after=$($excelProcessesAfter.Count)"
)
exit 0
