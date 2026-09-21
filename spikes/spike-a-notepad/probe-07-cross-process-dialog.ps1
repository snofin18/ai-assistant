<#
.SYNOPSIS
  probe-07: Cross-process Shell dialog (Notepad "Save As") parse + write.

.DESCRIPTION
  Spike A B1.4: close stage-0 DoD carry-over #5 -- go criterion
  "cross-process dialog parse success rate >= 90%".

  Modern Win11 Notepad uses WinUI3 MenuBar where keyboard shortcut
  Ctrl+Shift+S is NOT reliably delivered. We invoke File menu -> "Save As"
  via UIA InvokePattern. (Finding: submenu items appear as descendants of
  the Notepad window after expansion, not as separate top-level windows.)

.NOTES
  CONSTRAINT (ADR-0022 D1): Start-Process PID is never used.
  CONSTRAINT (ADR-0024 D4): PURE ASCII. CJK strings built via [char] code points.
  RESULT: D:\csart\eol-probe\RESULT-07.txt
#>

param(
  [string]$WorkDir = 'D:\csart\eol-probe',
  [int]$Iter   = 10,
  [int]$Warmup = 2
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class P07Win32 {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@

$logPath = Join-Path $WorkDir 'probe07-stdout.txt'
"" | Out-File -FilePath $logPath -Encoding UTF8
$logFile = [System.IO.StreamWriter]::new($logPath, $true, [System.Text.Encoding]::UTF8)
$logFile.AutoFlush = $true
function Log($msg) {
  $stamp = (Get-Date).ToString('HH:mm:ss')
  $line = "[$stamp] $msg"
  Write-Host $line
  $logFile.WriteLine($line)
}

# CJK string constants built from code points (ADR-0024 D4 PURE ASCII source)
$CN_FILE  = [char]0x6587 + [char]0x4EF6   # File menu = wen jian
$CN_SAVEAS = [char]0x53E6 + [char]0x5B58 + [char]0x4E3A   # Save As = ling cun wei

Log "probe-07 starting (iter=$Iter warmup=$Warmup) cn_file='$CN_FILE' cn_saveas='$CN_SAVEAS'"

# ---- helpers ----

function Stop-NotepadAll {
  Get-Process Notepad -ErrorAction SilentlyContinue | ForEach-Object {
    try { $_.CloseMainWindow() | Out-Null } catch {}
    Start-Sleep -Milliseconds 200
    if (-not $_.HasExited) { Stop-Process -Id $_.Id -Force }
  }
  Start-Sleep -Milliseconds 500
}

function New-NonceFile {
  param([string]$Dir, [string]$Tag, [string]$Content)
  if (-not (Test-Path $Dir)) { New-Item -ItemType Directory -Path $Dir -Force | Out-Null }
  $nonce = ([Guid]::NewGuid().ToString('N')).Substring(0, 8)
  $baseName = "probe07-${Tag}-${nonce}"
  $path = Join-Path $Dir ("${baseName}.txt")
  [System.IO.File]::WriteAllText($path, $Content)
  return @{ Path = $path; BaseName = $baseName; FileName = "${baseName}.txt" }
}

function Find-NotepadWindowByNonce {
  param([string]$Nonce)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $kids = $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($k in $kids) {
    if ($k.Current.Name -like ('*' + $Nonce + '*')) { return $k }
  }
  return $null
}

function Find-DocumentInWindow {
  param([System.Windows.Automation.AutomationElement]$Win)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $CT = [System.Windows.Automation.ControlType]
  $docCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Document)
  $editCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Edit)
  $orCond = New-Object System.Windows.Automation.OrCondition -ArgumentList @($docCond, $editCond)
  return $Win.FindFirst($TS::Descendants, $orCond)
}

function Set-Foreground {
  param([System.Windows.Automation.AutomationElement]$Win)
  $hwnd = [IntPtr]$Win.Current.NativeWindowHandle
  [P07Win32]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 300
}

function Close-Window {
  param([System.Windows.Automation.AutomationElement]$Win)
  try {
    $pat = $Win.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
    $pat.Close()
    Start-Sleep -Milliseconds 300
  } catch {}
}

function Trigger-SaveAs-Menu {
  param([System.Windows.Automation.AutomationElement]$NotepadWin)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $CT = [System.Windows.Automation.ControlType]
  $menuCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::MenuItem)
  $allMenus = $NotepadWin.FindAll($TS::Descendants, $menuCond)
  $fileMenu = $allMenus | Where-Object { $_.Current.Name -eq $CN_FILE } | Select-Object -First 1
  if (-not $fileMenu) { return $null }
  $fileMenu.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() | Out-Null
  Start-Sleep -Milliseconds 1000
  $allMenus2 = $NotepadWin.FindAll($TS::Descendants, $menuCond)
  $saveAs = $allMenus2 | Where-Object { $_.Current.Name -eq $CN_SAVEAS } | Select-Object -First 1
  if (-not $saveAs) {
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    return $null
  }
  $saveAs.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() | Out-Null
  Start-Sleep -Milliseconds 500
  return $true
}

function Find-SaveAsDialog {
  param([string]$Nonce)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $allKids = $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($k in $allKids) {
    $nm = $k.Current.Name
    if ($nm -and ($nm -like '*Save As*' -or $nm -like ('*' + $CN_SAVEAS + '*') -or $nm -like ('*' + $Nonce + '*'))) {
      if ($nm -like ('*' + $Nonce + '.txt - Notepad')) { continue }
      return $k
    }
  }
  return $null
}

function Find-FilenameInput {
  param([System.Windows.Automation.AutomationElement]$Dlg)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $CT = [System.Windows.Automation.ControlType]
  $editCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Edit)
  return $Dlg.FindFirst($TS::Descendants, $editCond)
}

function Find-SaveButton {
  param([System.Windows.Automation.AutomationElement]$Dlg)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $CT = [System.Windows.Automation.ControlType]
  $btnCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Button)
  $buttons = $Dlg.FindAll($TS::Descendants, $btnCond)
  foreach ($b in $buttons) {
    $nm = $b.Current.Name
    if ($nm -eq 'Save' -or $nm -eq ([char]0x4FDD + [char]0x5B58) -or $nm -eq ($CN_SAVEAS.Replace(([char]0x53E6 + [char]0x5B58 + [char]0x4E3A), ([char]0x4FDD + [char]0x5B58)))) { return $b }
  }
  return $null
}

function Get-Stat {
  param([double[]]$A)
  if ($A.Count -eq 0) { return [pscustomobject]@{ Count=0; Min=0; Median=0; Max=0 } }
  $s = $A | Sort-Object
  $n = $s.Count
  $med = if ($n % 2 -eq 1) { $s[[math]::Floor($n/2)] } else { ($s[$n/2-1] + $s[$n/2]) / 2 }
  return [pscustomobject]@{ Count=$n; Min=[math]::Round($s[0],2); Median=[math]::Round($med,2); Max=[math]::Round($s[$n-1],2) }
}

# ---- main ----

Stop-NotepadAll
$sourceContent = "B1.4 probe-07 cross-process Save As dialog PoC"

$dialogFound = @()
$editAccess = @()
$setFilename = @()
$saveClicked = @()
$fileOnDisk = @()
$dialogLatency = @()
$editLatency = @()
$saveLatency = @()

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  $isWarmup = $i -lt $Warmup
  $runId = $i + 1

  $src = New-NonceFile -Dir $WorkDir -Tag "src" -Content $sourceContent
  $srcPath = $src.Path
  $srcBaseName = $src.BaseName
  $tgt = New-NonceFile -Dir $WorkDir -Tag "tgt" -Content ""
  $tgtPath = $tgt.Path
  Remove-Item -Path $tgtPath -Force -ErrorAction SilentlyContinue

  try {
    Log "[iter $runId] launching notepad with $srcBaseName.txt"
    $null = Start-Process -FilePath 'notepad.exe' -ArgumentList "`"$srcPath`"" -PassThru

    $notepadWin = $null
    for ($poll = 0; $poll -lt 30; $poll++) {
      Start-Sleep -Milliseconds 500
      $notepadWin = Find-NotepadWindowByNonce -Nonce $srcBaseName
      if ($notepadWin) { break }
    }
    if (-not $notepadWin) { Log "[iter $runId] WARN: notepad window not found, skip"; continue }

    $doc = Find-DocumentInWindow -Win $notepadWin
    if (-not $doc) { Log "[iter $runId] WARN: document not found, skip"; Close-Window -Win $notepadWin; continue }

    # Trigger Save As via File menu Invoke
    $triggerOk = Trigger-SaveAs-Menu -NotepadWin $notepadWin
    if (-not $triggerOk) {
      Log "[iter $runId] WARN: File>SaveAs menu invoke failed, skip"
      Close-Window -Win $notepadWin
      continue
    }
    Log "[iter $runId] invoked File > SaveAs, waiting for dialog"

    # Poll for Save As dialog (different process, different title)
    $dlg = $null
    $dialogSw = [System.Diagnostics.Stopwatch]::StartNew()
    for ($poll = 0; $poll -lt 30; $poll++) {
      Start-Sleep -Milliseconds 500
      $dlg = Find-SaveAsDialog -Nonce $srcBaseName
      if ($dlg) { break }
    }
    $dialogSw.Stop()
    $dLat = [math]::Round($dialogSw.Elapsed.TotalMilliseconds, 2)

    if (-not $dlg) {
      Log "[iter $runId] WARN: Save As dialog not found after 15s"
      if (-not $isWarmup) { $dialogFound += $false }
      continue
    }
    if (-not $isWarmup) {
      $dialogFound += $true
      $dialogLatency += $dLat
    }
    Log "[iter $runId] dialog found in ${dLat}ms, name='$($dlg.Current.Name)' class='$($dlg.Current.ClassName)'"

    # Find FileName input
    $editSw = [System.Diagnostics.Stopwatch]::StartNew()
    $filenameInput = Find-FilenameInput -Dlg $dlg
    $editSw.Stop()
    $eLat = [math]::Round($editSw.Elapsed.TotalMilliseconds, 2)
    if (-not $filenameInput) {
      Log "[iter $runId] WARN: FileName input not found"
      if (-not $isWarmup) { $editAccess += $false }
      Close-Window -Win $dlg
      continue
    }
    if (-not $isWarmup) {
      $editAccess += $true
      $editLatency += $eLat
    }
    Log "[iter $runId] FileName input found in ${eLat}ms, ct=$($filenameInput.Current.ControlType.ProgrammaticName.Split('.')[-1])"

    # SetValue to new filename
    try {
      $vp = $filenameInput.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
      $vp.SetValue($tgt.FileName)
      Start-Sleep -Milliseconds 200
      if (-not $isWarmup) { $setFilename += $true }
      Log "[iter $runId] SetValue OK"
    } catch {
      Log "[iter $runId] SetValue FAIL: $($_.Exception.Message)"
      if (-not $isWarmup) { $setFilename += $false }
      Close-Window -Win $dlg
      continue
    }

    # Click Save button (or fallback to Enter)
    $saveBtn = Find-SaveButton -Dlg $dlg
    if (-not $saveBtn) {
      Log "[iter $runId] Save button not found, fallback to Enter key"
      Set-Foreground -Win $dlg
      [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
      Start-Sleep -Milliseconds 800
      if (-not $isWarmup) { $saveClicked += $true }
    } else {
      $saveSw = [System.Diagnostics.Stopwatch]::StartNew()
      try {
        $inv = $saveBtn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $inv.Invoke()
        Start-Sleep -Milliseconds 800
        $saveSw.Stop()
        $sLat = [math]::Round($saveSw.Elapsed.TotalMilliseconds, 2)
        if (-not $isWarmup) {
          $saveClicked += $true
          $saveLatency += $sLat
        }
        Log "[iter $runId] save button clicked in ${sLat}ms"
      } catch {
        Log "[iter $runId] save click FAIL: $($_.Exception.Message)"
        if (-not $isWarmup) { $saveClicked += $false }
      }
    }

    # Verify file on disk
    Start-Sleep -Milliseconds 500
    if (Test-Path $tgtPath) {
      $diskContent = Get-Content -Path $tgtPath -Raw -Encoding UTF8
      $diskOk = ($diskContent -eq $sourceContent)
      if (-not $isWarmup) { $fileOnDisk += $diskOk }
      Log "[iter $runId] file on disk: exists=$true, content_match=$diskOk"
    } else {
      if (-not $isWarmup) { $fileOnDisk += $false }
      Log "[iter $runId] file on disk: exists=$false"
    }

    Close-Window -Win $dlg
  } catch {
    Log "[iter $runId] ERROR: $($_.Exception.Message)"
  } finally {
    Get-Process Notepad -ErrorAction SilentlyContinue | ForEach-Object {
      try { $_.CloseMainWindow() | Out-Null } catch {}
      Start-Sleep -Milliseconds 200
      if (-not $_.HasExited) { Stop-Process -Id $_.Id -Force }
    }
    Remove-Item -Path $srcPath -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $tgtPath -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
  }
}

Stop-NotepadAll

# Count true (helper)
function Count-True { param($A) (($A | Where-Object { $_ -eq $true }).Count) }

$n = $Iter
$dfT = Count-True $dialogFound
$eaT = Count-True $editAccess
$sfT = Count-True $setFilename
$scT = Count-True $saveClicked
$fdT = Count-True $fileOnDisk
$dfPct = if ($n -gt 0) { [math]::Round($dfT * 100 / $n, 1) } else { 0 }
$eaPct = if ($n -gt 0) { [math]::Round($eaT * 100 / $n, 1) } else { 0 }
$sfPct = if ($n -gt 0) { [math]::Round($sfT * 100 / $n, 1) } else { 0 }
$scPct = if ($n -gt 0) { [math]::Round($scT * 100 / $n, 1) } else { 0 }
$fdPct = if ($n -gt 0) { [math]::Round($fdT * 100 / $n, 1) } else { 0 }

$minCount = (@($dfT, $eaT, $sfT, $scT, $fdT) | Measure-Object -Minimum).Minimum
$combinedPct = if ($n -gt 0) { [math]::Round($minCount * 100 / $n, 1) } else { 0 }
$overall = if ($n -gt 0 -and $combinedPct -ge 90) {'GO'} else {'NO-GO'}

$statDialog = Get-Stat $dialogLatency
$statEdit = Get-Stat $editLatency
$statSave = Get-Stat $saveLatency

$reportLines = @(
  'probe-07 cross-process Save As dialog - result',
  ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')),
  ('iter=' + $Iter + ' warmup=' + $Warmup),
  '',
  ('metric     | success count / ' + $Iter + ' | pct'),
  ('-----------+----------------------+------'),
  ('dialog_found   | ' + ('{0,4} / {1}' -f $dfT, $Iter) + '           | ' + $dfPct + '%'),
  ('edit_access    | ' + ('{0,4} / {1}' -f $eaT, $Iter) + '           | ' + $eaPct + '%'),
  ('set_filename   | ' + ('{0,4} / {1}' -f $sfT, $Iter) + '           | ' + $sfPct + '%'),
  ('save_clicked   | ' + ('{0,4} / {1}' -f $scT, $Iter) + '           | ' + $scPct + '%'),
  ('file_on_disk   | ' + ('{0,4} / {1}' -f $fdT, $Iter) + '           | ' + $fdPct + '%'),
  '',
  'latency_ms (only successful iterations):',
  ('  dialog_found  : ' + $statDialog.Count + ' samples; median ' + $statDialog.Median),
  ('  edit_access   : ' + $statEdit.Count + ' samples; median ' + $statEdit.Median),
  ('  save_clicked  : ' + $statSave.Count + ' samples; median ' + $statSave.Median),
  '',
  'go_criterion: cross_process_dialog_parse_success_rate >= 90%',
  '  defined as: ALL of (dialog_found AND edit_access AND set_filename AND save_clicked AND file_on_disk) must be true',
  ('  combined_pass_rate = {0} / {1} = {2}%' -f $minCount, $n, $combinedPct),
  ('  overall = ' + $overall)
)

$reportPath = Join-Path $WorkDir 'RESULT-07.txt'
[System.IO.File]::WriteAllLines($reportPath, $reportLines, [System.Text.Encoding]::UTF8)
Log "RESULT written to $reportPath"
foreach ($l in $reportLines) { Log $l }
$logFile.Close()
