param(
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [ValidateRange(5, 120)]
    [int]$CleanupTimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$summaryPath = Join-Path $evidence "summary.json"

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
    param([AllowNull()]$ComObject)
    if ($null -eq $ComObject) {
        return
    }
    try {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($ComObject)
    } catch {
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
        schema_version = 1
        mode = "excel-com-preflight"
        result = "failed"
        failure = "Existing EXCEL.EXE process detected; refusing to reuse or terminate it"
        existing_excel_process_ids = $existingProcessIds
        started_at_utc = $startedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    }
    Write-JsonFile -Path $summaryPath -Value $summary
    Write-Error $summary.failure
    exit 2
}

$excel = $null
$workbook = $null
$worksheet = $null
$rangeA1 = $null
$rangeA2 = $null
$rangeA3 = $null
$failure = $null
$cleanupFailures = New-Object System.Collections.Generic.List[string]
$taskkillExitCodes = New-Object System.Collections.Generic.List[string]
$newProcessIds = @()
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
    if ($formulaActual -ne 5) {
        throw "Formula preflight expected 5 but observed $formulaActual"
    }
} catch {
    $failure = $_.Exception.Message
} finally {
    if ($null -ne $workbook) {
        try {
            $workbook.Close($false)
        } catch {
            $cleanupFailures.Add("Workbook close failed: $($_.Exception.Message)")
        }
    }
    if ($null -ne $excel) {
        try {
            $excel.Quit()
        } catch {
            $cleanupFailures.Add("Excel Quit failed: $($_.Exception.Message)")
        }
    }
    Release-ComObject $rangeA3
    Release-ComObject $rangeA2
    Release-ComObject $rangeA1
    Release-ComObject $worksheet
    Release-ComObject $workbook
    Release-ComObject $excel
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
        $failure = "Probe-created EXCEL.EXE processes did not converge"
    }
}

$excelProcessesAfter = Get-ExcelProcesses
$completedAt = (Get-Date).ToUniversalTime().ToString("o")
$result = if ($failure -eq $null -and $excelProcessesAfter.Count -eq 0) {
    "passed"
} else {
    "failed"
}
$summary = [pscustomobject]@{
    schema_version = 1
    mode = "excel-com-preflight"
    result = $result
    failure = if ($failure -eq $null) { $null } else { [string]$failure }
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
        workbook_closed_without_save = $true
        cleanup_failures = @($cleanupFailures)
        taskkill_exit_codes = @($taskkillExitCodes)
        remaining_probe_process_ids = @($remainingProcessIds)
        excel_process_count_after = $excelProcessesAfter.Count
    }
    started_at_utc = $startedAt
    completed_at_utc = $completedAt
}
Write-JsonFile -Path $summaryPath -Value $summary

if ($result -ne "passed") {
    Write-Error ([string]$failure)
    exit 2
}

Write-Output (
    "excel-com-preflight passed version=$excelVersion build=$excelBuild " +
    "formula_value2=$formulaActual excel_processes_after=$($excelProcessesAfter.Count)"
)
exit 0
