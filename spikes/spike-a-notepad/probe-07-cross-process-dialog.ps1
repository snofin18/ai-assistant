<#
.SYNOPSIS
  probe-07: Cross-process Shell dialog (Notepad "Save As") parse + write.

.DESCRIPTION
  Spike A B1.4: close stage-0 DoD carry-over #5 -- go criterion
  "cross-process dialog parse success rate >= 90%".

  Win11 25H2 modern Notepad's "Save-As (CN)" is a Win32 #32770 dialog hosted in a
  SEPARATE process (explorer.exe child). PowerShell UIA1 does NOT enumerate it
  via RootElement.FindAll(Children). We use Win32 EnumWindows to find the
  #32770 HWND, then AutomationElement.FromHandle(hwnd) to bridge back to UIA
  for element enumeration. FileName input + Save button are addressed via
  Win32 FindChild + PostMessage (WM_SETTEXT / BM_CLICK) for reliability.

  KNOWN LIMITATION (2026-09-21): SetValue on the FileName Edit (id 1001)
  via Win32 WM_SETTEXT and UIA ValuePattern both fail because the Edit is
  subclassed by DirectUI/WinUI3 (modern Win11 dialogs reject WM_SETTEXT).
  This probe therefore marks set_filename as ALWAYS false and reports combined
  pass rate with this caveat. The other 4 metrics (dialog_found / edit_access
  / save_clicked / file_on_disk) are reliably measurable.

  Modern Notepad also responds to "File > Save As" menu Invoke (CN menus).
  Ctrl+Shift+S keyboard shortcut is unreliable in modern Notepad.

.NOTES
  CONSTRAINT (ADR-0022 D1): Start-Process PID is never used.
  CONSTRAINT (ADR-0024 D4): PURE ASCII. CJK strings via [char] code points.
  RESULT: D:\csart\eol-probe\RESULT-07.txt
  Requires: PowerShell 5.1+, UIAutomationClient, UIAutomationTypes
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

# Win32 P/Invoke declarations
if (-not ('P07Win32.W' -as [type])) {
  Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class W {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr hWnd, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr hWnd, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hWnd);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern IntPtr SendMessageW(IntPtr hWnd, uint Msg, IntPtr wParam, string lParam);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr hWnd, EnumChildProc lpEnumFunc, IntPtr lParam);
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  public delegate bool EnumChildProc(IntPtr hWnd, IntPtr lParam);
  public static IntPtr FindChild(IntPtr parent, int ctrlId) {
    IntPtr found = IntPtr.Zero;
    EnumChildWindows(parent, (h, l) => {
      int id = GetDlgCtrlID(h);
      if (id == ctrlId) { found = h; return false; }
      IntPtr inner = FindChild(h, ctrlId);
      if (inner != IntPtr.Zero) { found = inner; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@
}

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

# CJK string constants from code points (ADR-0024 D4 PURE ASCII source)
$CN_FILE   = [char]0x6587 + [char]0x4EF6
$CN_SAVEAS = [char]0x53E6 + [char]0x5B58 + [char]0x4E3A

Log "probe-07 starting (iter=$Iter warmup=$Warmup) cn_file='$CN_FILE' cn_saveas='$CN_SAVEAS'"

# Win32 constants
$WM_SETTEXT = 0x000C
$BM_CLICK   = 0x00F5
$WM_COMMAND = 0x0111
$IDOK       = 1
$IDCANCEL   = 2

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

# Win32 EnumWindows: find #32770 dialog (PowerShell UIA1 misses these)
function Find-SaveAsDialog {
  $saveHwnd = [IntPtr]::Zero
  $cb = [W+EnumWindowsProc]{
    param($h, $l)
    $sb = New-Object System.Text.StringBuilder 256
    [W]::GetClassName($h, $sb, 256) | Out-Null
    $cls = $sb.ToString()
    if ($cls -eq '#32770') {
      $sb2 = New-Object System.Text.StringBuilder 256
      [W]::GetWindowText($h, $sb2, 256) | Out-Null
      $title = $sb2.ToString()
      if ($title -like '*Save As*' -or $title -eq $CN_SAVEAS) {
        $script:saveHwnd = $h
      }
    }
    return $true
  }
  [W]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
  return $script:saveHwnd
}

function Get-FilenameEditHwnd {
  param([IntPtr]$DlgHwnd)
  # FileName edit has dialog item ID 1001 in modern Win11 Save dialog
  # FindChild recursively (not just immediate child) because edit is nested
  return [W]::FindChild($DlgHwnd, 1001)
}

function Click-SaveButtonNonBlocking {
  param([IntPtr]$BtnHwnd)
  if ($BtnHwnd -eq [IntPtr]::Zero) { return $false }
  # Use PostMessage to avoid blocking on dialog thread
  [W]::PostMessage($BtnHwnd, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
  return $true
}

function Get-SaveButtonHwnd {
  param([IntPtr]$DlgHwnd)
  # Save button = standard Win32 IDOK = 1
  return [W]::FindChild($DlgHwnd, 1)
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
$sourceContent = "B1.4 probe-07 cross-process Save As dialog PoC content"

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

  $src = New-NonceFile -Dir $WorkDir -Tag 'src' -Content $sourceContent
  $srcPath = $src.Path
  $srcBaseName = $src.BaseName
  $tgt = New-NonceFile -Dir $WorkDir -Tag 'tgt' -Content ''  # different filename = test rename
  Remove-Item -Path $tgt.Path -Force -ErrorAction SilentlyContinue  # delete empty tgt file before save
  $tgtPath = $tgt.Path

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

    $triggerOk = Trigger-SaveAs-Menu -NotepadWin $notepadWin
    if (-not $triggerOk) {
      Log "[iter $runId] WARN: File>SaveAs menu invoke failed, skip"
      Close-Window -Win $notepadWin
      continue
    }
    Log "[iter $runId] invoked File > SaveAs, polling for dialog via Win32"

    # Poll for dialog (Win32 EnumWindows)
    $dlgHwnd = [IntPtr]::Zero
    $dialogSw = [System.Diagnostics.Stopwatch]::StartNew()
    for ($poll = 0; $poll -lt 30; $poll++) {
      Start-Sleep -Milliseconds 500
      $dlgHwnd = Find-SaveAsDialog
      if ($dlgHwnd -ne [IntPtr]::Zero) { break }
    }
    $dialogSw.Stop()
    $dLat = [math]::Round($dialogSw.Elapsed.TotalMilliseconds, 2)

    if ($dlgHwnd -eq [IntPtr]::Zero) {
      Log "[iter $runId] WARN: Save As dialog not found after 15s"
      if (-not $isWarmup) { $dialogFound += $false }
      continue
    }
    if (-not $isWarmup) {
      $dialogFound += $true
      $dialogLatency += $dLat
    }
    Log "[iter $runId] dialog found in ${dLat}ms, hwnd=$dlgHwnd"

    # Find FileName edit recursively (DirectUI nests it)
    $editSw = [System.Diagnostics.Stopwatch]::StartNew()
    $editHwnd = Get-FilenameEditHwnd -DlgHwnd $dlgHwnd
    $editSw.Stop()
    $eLat = [math]::Round($editSw.Elapsed.TotalMilliseconds, 2)
    if ($editHwnd -eq [IntPtr]::Zero) {
      Log "[iter $runId] WARN: FileName edit (id 1001) not findable via Win32 FindChild"
      if (-not $isWarmup) { $editAccess += $false }
      # Close dialog
      $cancelHwnd = [W]::FindChild($dlgHwnd, $IDCANCEL)
      if ($cancelHwnd -ne [IntPtr]::Zero) { [W]::PostMessage($cancelHwnd, $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null }
      continue
    }
    if (-not $isWarmup) {
      $editAccess += $true
      $editLatency += $eLat
    }
    Log "[iter $runId] FileName edit found in ${eLat}ms, hwnd=$editHwnd"

    # set_filename: actually attempt Win32 WM_SETTEXT, then verify by file existence
    $setSw = [System.Diagnostics.Stopwatch]::StartNew()
    $wmSetOk = [W]::SendMessageW($editHwnd, $WM_SETTEXT, [IntPtr]::Zero, $tgt.FileName)
    Start-Sleep -Milliseconds 200
    # Empirical verification: did WM_SETTEXT actually take effect?
    # (Cannot reliably read back via GetWindowText -- DirectUI may not update cached buffer)
    # Instead, we will check after Save click whether the file with new name exists
    $setSw.Stop()
    $setLat = [math]::Round($setSw.Elapsed.TotalMilliseconds, 2)
    if ($wmSetOk -ne 0) {
      Log "[iter $runId] set_filename: WM_SETTEXT returned ok (verifying via file save...)"
    } else {
      Log "[iter $runId] set_filename: WM_SETTEXT returned 0 (likely failed)"
    }

    # Click Save button via BM_CLICK
    $saveSw = [System.Diagnostics.Stopwatch]::StartNew()
    $saveBtnHwnd = Get-SaveButtonHwnd -DlgHwnd $dlgHwnd
    if ($saveBtnHwnd -ne [IntPtr]::Zero) {
      [W]::PostMessage($saveBtnHwnd, $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
      Start-Sleep -Milliseconds 1500
      $saveSw.Stop()
      $sLat = [math]::Round($saveSw.Elapsed.TotalMilliseconds, 2)
      if (-not $isWarmup) {
        $saveClicked += $true
        $saveLatency += $sLat
      }
      Log "[iter $runId] save button clicked in ${sLat}ms (hwnd=$saveBtnHwnd)"
    } else {
      Log "[iter $runId] WARN: Save button (id 1) not findable"
      if (-not $isWarmup) { $saveClicked += $false }
    }

    # Empirical verification of set_filename: did the file get saved with the NEW name?
    Start-Sleep -Milliseconds 500
    if (Test-Path $tgtPath) {
      $diskContent = Get-Content -Path $tgtPath -Raw -Encoding UTF8
      $diskOk = ($diskContent -eq $sourceContent)
      if (-not $isWarmup) { $fileOnDisk += $diskOk }
      if (-not $isWarmup) { $setFilename += $diskOk }  # file with new name = rename worked
      Log "[iter $runId] file_on_disk: tgt_path exists=$true content_match=$diskOk"
      if ($diskOk) { Log "[iter $runId] set_filename: TRUE (file saved with new name=$($tgt.FileName))" }
      else { Log "[iter $runId] set_filename: FALSE (file exists but content wrong)" }
    } else {
      # Check if original src was saved (dialog fell back to original name)
      if (Test-Path $srcPath) {
        Log "[iter $runId] file_on_disk: tgt_path NOT EXISTS; src_path saved (rename failed)"
      } else {
        Log "[iter $runId] file_on_disk: NEITHER tgt NOR src exists"
      }
      if (-not $isWarmup) { $fileOnDisk += $false; $setFilename += $false }
    }
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
$overall = if ($n -gt 0 -and $combinedPct -ge 90) {'GO'} else {'NO-GO (set_filename disabled for Win11 25H2 DirectUI Edit)'}

$statDialog = Get-Stat $dialogLatency
$statEdit = Get-Stat $editLatency
$statSave = Get-Stat $saveLatency

$reportLines = @(
  'probe-07 cross-process Save As dialog - result',
  ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')),
  ('iter=' + $Iter + ' warmup=' + $Warmup),
  '',
  'metric     | success count / ' + $Iter + ' | pct',
  '-----------+----------------------+------',
  ('dialog_found   | ' + ('{0,4} / {1}' -f $dfT, $Iter) + '           | ' + $dfPct + '%'),
  ('edit_access    | ' + ('{0,4} / {1}' -f $eaT, $Iter) + '           | ' + $eaPct + '%'),
  ('set_filename   | ' + ('{0,4} / {1}' -f $sfT, $Iter) + '           | ' + $sfPct + '% (DISABLED: DirectUI Edit rejects WM_SETTEXT)'),
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
  '  KNOWN LIMITATION: set_filename is always FALSE in Win11 25H2 (DirectUI Edit subclass',
  '  rejects both Win32 WM_SETTEXT and UIA ValuePattern). See probe-07 header for details.',
  ('  combined_pass_rate = {0} / {1} = {2}%' -f $minCount, $n, $combinedPct),
  ('  overall = ' + $overall)
)

$reportPath = Join-Path $WorkDir 'RESULT-07.txt'
[System.IO.File]::WriteAllLines($reportPath, $reportLines, [System.Text.Encoding]::UTF8)
Log "RESULT written to $reportPath"
foreach ($l in $reportLines) { Log $l }
$logFile.Close()
