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
    [string]$CancellationCheckpoint = "after-workbook-create",

    [ValidateSet("none", "after-com-create", "after-workbook-create", "after-formula")]
    [string]$HangAfterCheckpoint = "none"
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$summaryPath = Join-Path $evidence "summary.json"
$ownershipPath = Join-Path $evidence "ownership.json"
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
    if ($HangAfterCheckpoint -eq $Checkpoint) {
        Start-Sleep -Seconds 300
    }
}

function Wait-ExcelProcessIdentitiesGone {
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
        if ((Get-ExcelProcesses).Count -eq 0) {
            return $true
        }
        Start-Sleep -Milliseconds 250
    }
    return (Get-ExcelProcesses).Count -eq 0
}

$startedAt = (Get-Date).ToUniversalTime().ToString("o")
$existingProcesses = Get-ExcelProcesses
$existingProcessIds = @($existingProcesses | ForEach-Object { [int]$_.Id })
$existingIdentities = @(
    $existingProcesses |
        ForEach-Object { Get-ProcessIdentity -ProcessId ([int]$_.Id) } |
        Where-Object { $null -ne $_ }
)
if ($existingProcesses.Count -ne 0) {
    $existingProcessesAfterRefusal = Get-ExcelProcesses
    $existingProcessIdsAfterRefusal = @(
        $existingProcessesAfterRefusal |
            ForEach-Object { [int]$_.Id }
    )
    $stillAliveIdentities = @(
        $existingIdentities |
            Where-Object { Test-ProcessIdentity -Identity $_ }
    )
    Write-JsonFile -Path $ownershipPath -Value ([pscustomobject]@{
        schema_version = 1
        state = "refused-existing-process"
        existing_process_identities = $existingIdentities
        observed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    })
    $summary = [pscustomobject]@{
        schema_version = 2
        mode = "excel-com-preflight"
        result = "failed"
        failure_kind = "existing_process"
        failure = "Existing EXCEL.EXE process detected; refusing to reuse or terminate it"
        failure_injection_point = $FailureInjectionPoint
        cancellation_signal_path = $cancellationSignal
        cancellation_checkpoint = $CancellationCheckpoint
        hang_after_checkpoint = $HangAfterCheckpoint
        existing_excel_process_ids = $existingProcessIds
        existing_excel_process_ids_after_refusal = $existingProcessIdsAfterRefusal
        existing_excel_process_identities_still_alive = $stillAliveIdentities
        cleanup = [pscustomobject]@{
            cleanup_succeeded = $true
            probe_created_any_process = $false
            all_probe_processes_reaped = $true
            excel_process_count_after = $existingProcessesAfterRefusal.Count
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
$ownedIdentity = $null
$taskkillIdentityFailures = New-Object System.Collections.Generic.List[string]
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
    $ownedProcessId = Get-ExcelApplicationProcessId -Excel $excel
    $ownedIdentity = Get-ProcessIdentity -ProcessId $ownedProcessId
    if (
        $null -eq $ownedIdentity -or
        $ownedIdentity.process_name -ne "EXCEL"
    ) {
        throw "Excel COM started without a verifiable EXCEL.EXE process identity"
    }
    $newProcessIds = @([int]$ownedIdentity.id)
    Write-JsonFile -Path $ownershipPath -Value ([pscustomobject]@{
        schema_version = 1
        state = "owned"
        established_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        process_identity = $ownedIdentity
    })
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

    $remainingIdentities = Wait-ExcelProcessIdentitiesGone `
        -Identities @($ownedIdentity) `
        -TimeoutSeconds $CleanupTimeoutSeconds
    foreach ($remainingIdentity in $remainingIdentities) {
        $taskkillResult = Invoke-VerifiedProcessTreeTermination -Identity $remainingIdentity
        if ($null -ne $taskkillResult.exit_code) {
            $taskkillExitCodes.Add([string]$taskkillResult.exit_code)
        }
        if ($null -ne $taskkillResult.error) {
            $taskkillIdentityFailures.Add(
                "PID $($remainingIdentity.id): $($taskkillResult.error)"
            )
        }
    }
    if ($remainingIdentities.Count -ne 0) {
        $remainingIdentities = Wait-ExcelProcessIdentitiesGone `
            -Identities $remainingIdentities `
            -TimeoutSeconds $CleanupTimeoutSeconds
    }
    if ($remainingIdentities.Count -ne 0 -and $failure -eq $null) {
        $failureKind = "cleanup_failure"
        $failure = "Probe-created EXCEL.EXE processes did not converge"
    }
    $remainingProcessIds = @($remainingIdentities | ForEach-Object { [int]$_.id })
    Write-JsonFile -Path $ownershipPath -Value ([pscustomobject]@{
        schema_version = 1
        state = if ($remainingIdentities.Count -eq 0) { "reaped" } else { "cleanup_failed" }
        established_at_utc = $startedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        process_identity = $ownedIdentity
        remaining_process_ids = $remainingProcessIds
    })
}

[void](Wait-ExcelProcessCountZero -TimeoutSeconds $CleanupTimeoutSeconds)
$excelProcessesAfter = Get-ExcelProcesses
$completedAt = (Get-Date).ToUniversalTime().ToString("o")
$allProbeProcessesReaped = (
    $null -eq $ownedIdentity -or
    -not (Test-ProcessIdentity -Identity $ownedIdentity)
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
    hang_after_checkpoint = $HangAfterCheckpoint
    cancellation_signal_observed = $cancellationObserved
    excel_version = $excelVersion
    excel_build = $excelBuild
    excel_path = $excelPath
    probe_excel_process_ids = $newProcessIds
    owned_process_identity = $ownedIdentity
    ownership_path = $ownershipPath
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
        taskkill_identity_failures = @($taskkillIdentityFailures)
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
