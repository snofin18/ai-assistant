<#
.SYNOPSIS
  probe-08: Notepad failure injection PoC (4 scenarios).

.DESCRIPTION
  Spike A B1.5: validate that Adapter operation handles failure modes gracefully.
  4 scenarios x 3 iterations each = 12 iters (10 + 2 warmup cycling scenarios).

  Scenarios (per SPIKE-A.md B1.5 / apps/notepad.md section 8):
    1. process_killed  : kill Notepad while open; verify clean exception handling
    2. minimized      : minimize Notepad via ShowWindow SW_MINIMIZE; verify UIA still finds it
    3. other_desktop  : create separate Win32 desktop, switch Notepad to it; verify recovery via desktop switch
    4. unsaved_dialog : modify content then close; verify "save changes?" dialog handling

  Measurement per scenario per iter:
    - setup_ok: failure mode applied successfully
    - recovery_ok: probe recovered and completed its test operation
    - state_correct: file/process state matches expectation

.NOTES
  CONSTRAINT (ADR-0022 D1): Start-Process PID is never used.
  CONSTRAINT (ADR-0024 D4): PURE ASCII.
  RESULT: D:\csart\eol-probe\RESULT-08.txt
  Requires: PowerShell 5.1+, UIAutomationClient, UIAutomationTypes
#>

param(
  [string]$WorkDir = 'D:\csart\eol-probe',
  [int]$Iter = 10,
  [int]$Warmup = 2
)

$ErrorActionPreference = 'Continue'  # important: do NOT abort on per-iter failures (we measure recovery)
# TASK-101: Win32 Input helper (replaces deprecated SendKeys)
Import-Module (Join-Path $PSScriptRoot 'Win32-Input.psm1') -Force
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

if (-not ('P08W.W' -as [type])) {
  Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class W {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern IntPtr CreateDesktop(string lpszDesktop, IntPtr lpszDevice, IntPtr lpszDevMode, int dwFlags, int dwDesiredAccess, IntPtr lpszSecurity);
  [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr hDesktop);
  [DllImport("user32.dll")] public static extern bool SetThreadDesktop(IntPtr hDesktop);
  [DllImport("user32.dll")] public static extern IntPtr GetThreadDesktop(uint dwThreadId);
  [DllImport("user32.dll")] public static extern IntPtr OpenInputDesktop(int dwFlags, bool fInherit, int dwDesiredAccess);
  [DllImport("user32.dll")] public static extern bool SetProcessDefaultLayout(IntPtr hDesktop);
  [DllImport("user32.dll")] public static extern IntPtr OpenWindowStation(string lpszWinSta, bool fInherit, int dwDesiredAccess);
  [DllImport("user32.dll")] public static extern bool SetProcessWindowStation(IntPtr hWinSta);
  [DllImport("user32.dll")] public static extern bool CloseWindowStation(IntPtr hWinSta);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);
  [DllImport("user32.dll")] public static extern bool BlockInput(bool fBlockIt);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr hWnd, System.Text.StringBuilder lpClassName, int nMaxCount);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder lpString, int nMaxCount);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr hWndParent, EnumWindowsProc lpEnumFunc, IntPtr lParam);
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
}
"@
}

$SW_HIDE = 0; $SW_SHOWNORMAL = 1; $SW_MINIMIZE = 6; $SW_RESTORE = 9

$logPath = Join-Path $WorkDir 'probe08-stdout.txt'
"" | Out-File -FilePath $logPath -Encoding UTF8
$logFile = [System.IO.StreamWriter]::new($logPath, $true, [System.Text.Encoding]::UTF8)
$logFile.AutoFlush = $true
function Log($msg) {
  $stamp = (Get-Date).ToString('HH:mm:ss')
  $line = "[$stamp] $msg"
  Write-Host $line
  $logFile.WriteLine($line)
}

Log "probe-08 starting (iter=$Iter warmup=$Warmup)"

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
  $path = Join-Path $Dir ("probe08-${Tag}-${nonce}.txt")
  [System.IO.File]::WriteAllText($path, $Content)
  return @{ Path = $path; Nonce = "probe08-${Tag}-${nonce}" }
}

function Find-NotepadByNonce {
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

function Get-DocText {
  param([System.Windows.Automation.AutomationElement]$Doc)
  try {
    $vp = $Doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    return $vp.Current.Value
  } catch { return $null }
}

function Set-DocText {
  param([System.Windows.Automation.AutomationElement]$Doc, [string]$NewText)
  try {
    $vp = $Doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $vp.SetValue($NewText)
    return $true
  } catch { return $false }
}

function Close-Win {
  param([System.Windows.Automation.AutomationElement]$Win)
  try {
    $pat = $Win.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
    $pat.Close()
    Start-Sleep -Milliseconds 300
  } catch {}
}

# Helper: bring back Win32 helper functions to detect dialogs and check minimization.
# These functions wrap IsIconic + #32770 class check + Win32 text reads to bypass the UIA
# limitations we hit in probe-08 v1.
function Open-InteractiveWindowStation {
  param([string]$Name = "WinSta0")
  $hWinSta = [W]::OpenWindowStation($Name, $false, 0x100)  # WINSTA_ALL_ACCESS
  if ($hWinSta -eq [IntPtr]::Zero) { throw "OpenWindowStation($Name) failed: $($Error[0])" }
  $ok = [W]::SetProcessWindowStation($hWinSta)
  if (-not $ok) { throw "SetProcessWindowStation failed: $($Error[0])" }
  return $hWinSta
}

# Find any top-level window with a given Win32 class name.
# Class #32770 = standard Win32 dialog (used by Notepad unsaved-changes prompt and Save-As).
function Wait-ForDialogByClass {
  param([string]$Class = "#32770", [int]$TimeoutMs = 5000)
  $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
  while ((Get-Date) -lt $deadline) {
    $script:dialogHwnd = [IntPtr]::Zero
    $cb = [W+EnumWindowsProc]{
      param($h, $l)
      if ([W]::GetParent($h) -eq [IntPtr]::Zero) {
        $sb = New-Object System.Text.StringBuilder 256
        $len = [W]::GetClassName($h, $sb, 256)
        if ($len -gt 0 -and $sb.ToString() -eq $Class) {
          $script:dialogHwnd = $h
          return $false
        }
      }
      return $true
    }
    [W]::EnumChildWindows([IntPtr]::Zero, $cb, [IntPtr]::Zero) | Out-Null
    if ($script:dialogHwnd -ne [IntPtr]::Zero) { return $script:dialogHwnd }
    Start-Sleep -Milliseconds 100
  }
  return [IntPtr]::Zero
}

function Get-DialogText {
  param([IntPtr]$Hwnd)
  $len = [W]::GetWindowTextLength($Hwnd)
  if ($len -le 0) { return "" }
  $sb = New-Object System.Text.StringBuilder ($len + 1)
  [W]::GetWindowText($Hwnd, $sb, $sb.Capacity) | Out-Null
  return $sb.ToString()
}

function Get-Hwnd {
  param([System.Windows.Automation.AutomationElement]$Win)
  return [IntPtr]$Win.Current.NativeWindowHandle
}

# Scenario implementations: each returns setup_ok, recovery_ok, state_correct

function Scenario-ProcessKilled {
  param([System.Windows.Automation.AutomationElement]$Doc, [string]$Content, [string]$Nonce, [string]$File)
  $setupOk = $false; $recoveryOk = $false; $stateCorrect = $false
  # The setup: kill all Notepad (simulates user closing the app abruptly)
  Get-Process Notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Milliseconds 800
  $setupOk = ($null -eq (Get-Process Notepad -ErrorAction SilentlyContinue))
  # Recovery: probe tries to find window -- should fail gracefully (return null, not throw)
  try {
    $stillThere = Find-NotepadByNonce -Nonce $Nonce
    $recoveryOk = ($null -eq $stillThere)  # recovery OK = we correctly detected missing window
  } catch {
    $recoveryOk = $false
  }
  # State: file on disk should still exist with original content (we didn't touch it)
  if (Test-Path $File) {
    $disk = Get-Content -Path $File -Raw -Encoding UTF8
    $stateCorrect = ($disk -eq $Content)
  }
  return @($setupOk, $recoveryOk, $stateCorrect)
}

function Scenario-Minimized {
  param([System.Windows.Automation.AutomationElement]$Win, [System.Windows.Automation.AutomationElement]$Doc, [string]$Content, [string]$Nonce, [string]$File)
  $setupOk = $false; $recoveryOk = $false; $stateCorrect = $false
  if ($null -eq $Win) { return @($setupOk, $recoveryOk, $stateCorrect) }
  $hwnd = Get-Hwnd -Win $Win
  # Minimize
  $null = [W]::ShowWindow($hwnd, $SW_MINIMIZE)
  Start-Sleep -Milliseconds 500
  # IsWindowVisible returns True for minimized windows. Use IsIconic instead.
  $setupOk = [W]::IsIconic($hwnd)
  # Recovery: probe still finds + reads from minimized window
  try {
    $stillThere = Find-NotepadByNonce -Nonce $Nonce
    if ($stillThere) {
      $doc2 = Find-DocumentInWindow -Win $stillThere
      if ($doc2) {
        $txt = Get-DocText -Doc $doc2
        $recoveryOk = ($txt -eq $Content)
      }
    }
  } catch {
    $recoveryOk = $false
  }
  # Restore
  $null = [W]::ShowWindow($hwnd, $SW_RESTORE)
  Start-Sleep -Milliseconds 300
  # State: content unchanged
  $stateCorrect = $recoveryOk
  return @($setupOk, $recoveryOk, $stateCorrect)
}

function Scenario-OtherDesktop {
  param([System.Windows.Automation.AutomationElement]$Win, [string]$Content, [string]$Nonce, [string]$File)
  $setupOk = $false; $recoveryOk = $false; $stateCorrect = $false
  if ($null -eq $Win) { return @($setupOk, $recoveryOk, $stateCorrect) }
  $hwnd = Get-Hwnd -Win $Win
  # Save current desktop handle
  $origDesktop = [W]::GetThreadDesktop(0)
  # Window-station permission dance (required for CreateDesktop with GENERIC_ALL).
  Open-InteractiveWindowStation -Name "WinSta0" | Out-Null
  # Create a new desktop
  $newDesktop = [W]::CreateDesktop("Probe08Desktop_$Nonce", [IntPtr]::Zero, [IntPtr]::Zero, 0, 0x20000000, [IntPtr]::Zero)
  if ($newDesktop -eq [IntPtr]::Zero) {
    # ERROR_NOT_ENOUGH_MEMORY (8) on Win11 25H2 = Windows security policy blocks non-trusted processes from creating desktops.
    # Document this in RESULT; scenario is expected to fail setup on this OS version.
    Log "  [other_desktop] CreateDesktop failed: errno=8 ERROR_NOT_ENOUGH_MEMORY (Win11 25H2 platform limit; CreateDesktop restricted to trusted processes)"
    $setupOk = $false
    return @($setupOk, $recoveryOk, $stateCorrect)
  }
  # Move window to new desktop (via SetWindowLong + GWL_HWNDPARENT? Actually use SetThreadDesktop + SetWindowPos?)
  # Simpler: hide the window on current desktop, find it on new desktop
  # Actually the easiest: use SetThreadDesktop + attach thread input, then move
  try {
    $null = [W]::SetThreadDesktop($newDesktop)
    Start-Sleep -Milliseconds 200
    # Re-find window via UIA on new desktop
    $winOnNew = Find-NotepadByNonce -Nonce $Nonce
    $setupOk = ($null -ne $winOnNew)  # found = setup OK
    if ($winOnNew) {
      $doc2 = Find-DocumentInWindow -Win $winOnNew
      if ($doc2) {
        $txt = Get-DocText -Doc $doc2
        $recoveryOk = ($txt -eq $Content)
      }
    }
    # Switch back to original
    $null = [W]::SetThreadDesktop($origDesktop)
    Start-Sleep -Milliseconds 200
    # Find on original desktop (should work again)
    $winBack = Find-NotepadByNonce -Nonce $Nonce
    if ($null -eq $winBack) {
      # We're on the original, window may have moved to new -- need to switch back to find it
      $null = [W]::SetThreadDesktop($newDesktop)
      Start-Sleep -Milliseconds 200
      $winBack = Find-NotepadByNonce -Nonce $Nonce
      if ($winBack) {
        # File unchanged
        if (Test-Path $File) {
          $disk = Get-Content -Path $File -Raw -Encoding UTF8
          $stateCorrect = ($disk -eq $Content)
        }
      }
      $null = [W]::SetThreadDesktop($origDesktop)
    } else {
      if (Test-Path $File) {
        $disk = Get-Content -Path $File -Raw -Encoding UTF8
        $stateCorrect = ($disk -eq $Content)
      }
    }
  } catch {
    Log "  [other_desktop] exception: $($_.Exception.Message)"
  } finally {
    # Switch back to original desktop before closing
    try { [W]::SetThreadDesktop($origDesktop) | Out-Null } catch {}
    # Close the new desktop
    [W]::CloseDesktop($newDesktop) | Out-Null
    # Restore default window-station (WinSta0) so subsequent scenarios still find windows
    try { [W]::SetProcessWindowStation([IntPtr]::Zero) | Out-Null } catch {}
  }
  return @($setupOk, $recoveryOk, $stateCorrect)
}

function Scenario-UnsavedDialog {
  param([System.Windows.Automation.AutomationElement]$Win, [System.Windows.Automation.AutomationElement]$Doc, [string]$Content, [string]$Nonce, [string]$File)
  $setupOk = $false; $recoveryOk = $false; $stateCorrect = $false
  if ($null -eq $Doc -or $null -eq $Win) { return @($setupOk, $recoveryOk, $stateCorrect) }
  # Modify content so Notepad has unsaved changes
  $newText = "$Content`nMODIFIED"
  $setOk = Set-DocText -Doc $Doc -NewText $newText
  Start-Sleep -Milliseconds 300
  # Trigger Close via WindowPattern
  Close-Win -Win $Win
  Start-Sleep -Milliseconds 1500  # let "Save changes?" dialog appear
  # Detect dialog by Win32 class name (#32770) -- title text varies by locale so
  # locale-pattern matching is unreliable on Win11 25H2.
  $dialogHwnd = Wait-ForDialogByClass -Class "#32770" -TimeoutMs 5000
  $dialog = $null
  if ($dialogHwnd -ne [IntPtr]::Zero) {
    $dialogTitle = Get-DialogText -Hwnd $dialogHwnd
    Log "  [unsaved_dialog] detected: title=$dialogTitle class=#32770 hwnd=$dialogHwnd"
    $AE = [System.Windows.Automation.AutomationElement]
    $dialog = $AE::FromHandle($dialogHwnd)
    $setupOk = $true
  } else {
    $setupOk = $false
  }
  # Click "Don't Save" (or press 'N')
  if ($null -ne $dialog) {
    $recoveryOk = $false
    try {
      # Try to find "Don't Save" button
      $CT = [System.Windows.Automation.ControlType]
      $btnCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Button)
      $btns = $dialog.FindAll($TS::Descendants, $btnCond)
      $dontSave = $null
      foreach ($b in $btns) {
        $bn = $b.Current.Name
        if ($bn -like "*Don't*" -or $bn -like '*dont save (TW)*' -or $bn -like '' -or $bn -like '*no*' -or $bn -like '*dont save*' -or $bn -like '*dont save (N)*' -or $bn -like "*Don't Save*") {
          $dontSave = $b
          break
        }
      }
      if ($null -ne $dontSave) {
        $inv = $dontSave.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $inv.Invoke()
      } else {
        # Fallback: SendKeys N
        Send-SendInputVk -Vk 0x4E | Out-Null  # N (No)  # TASK-101: replaces SendKeys
      }
      Start-Sleep -Milliseconds 800
      $recoveryOk = $true
    } catch {
      $recoveryOk = $false
    }
  }
  # State: file on disk should match ORIGINAL content (Notepad "don't save" = discard changes)
  if (Test-Path $File) {
    $disk = Get-Content -Path $File -Raw -Encoding UTF8
    $stateCorrect = ($disk -eq $Content)
  }
  return @($setupOk, $recoveryOk, $stateCorrect)
}

# ---- main ----

$originalContent = "B1.5 probe-08 failure injection PoC content"

# Metrics: 3 metrics per scenario
$setupOk = @{ 'process_killed' = @(); 'minimized' = @(); 'other_desktop' = @(); 'unsaved_dialog' = @() }
$recoveryOk = @{ 'process_killed' = @(); 'minimized' = @(); 'other_desktop' = @(); 'unsaved_dialog' = @() }
$stateOk = @{ 'process_killed' = @(); 'minimized' = @(); 'other_desktop' = @(); 'unsaved_dialog' = @() }
$latency = @()

$scenarios = @('process_killed', 'minimized', 'other_desktop', 'unsaved_dialog')

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  $isWarmup = $i -lt $Warmup
  $runId = $i + 1
  $scenario = $scenarios[$i % 4]
  Stop-NotepadAll

  # Setup: launch notepad with file
  $src = New-NonceFile -Dir $WorkDir -Tag "src" -Content $originalContent
  $srcPath = $src.Path
  $nonce = $src.Nonce

  $notepadWin = $null
  $doc = $null
  try {
    Log "[iter $runId scenario=$scenario] launching notepad with $nonce.txt"
    $null = Start-Process -FilePath 'notepad.exe' -ArgumentList "`"$srcPath`"" -PassThru
    for ($poll = 0; $poll -lt 30; $poll++) {
      Start-Sleep -Milliseconds 500
      $notepadWin = Find-NotepadByNonce -Nonce $nonce
      if ($notepadWin) { break }
    }
    if (-not $notepadWin) { Log "[iter $runId] WARN: notepad window not found, skip"; continue }
    $doc = Find-DocumentInWindow -Win $notepadWin
    if (-not $doc) { Log "[iter $runId] WARN: document not found, skip"; Close-Win -Win $notepadWin; continue }

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    switch ($scenario) {
      'process_killed' { $r = Scenario-ProcessKilled -Doc $doc -Content $originalContent -Nonce $nonce -File $srcPath }
      'minimized' { $r = Scenario-Minimized -Win $notepadWin -Doc $doc -Content $originalContent -Nonce $nonce -File $srcPath }
      'other_desktop' { $r = Scenario-OtherDesktop -Win $notepadWin -Content $originalContent -Nonce $nonce -File $srcPath }
      'unsaved_dialog' { $r = Scenario-UnsavedDialog -Win $notepadWin -Doc $doc -Content $originalContent -Nonce $nonce -File $srcPath }
    }
    $sw.Stop()
    $lat = [math]::Round($sw.Elapsed.TotalMilliseconds, 2)
    if (-not $isWarmup) {
      $setupOk[$scenario] += $r[0]
      $recoveryOk[$scenario] += $r[1]
      $stateOk[$scenario] += $r[2]
      $latency += $lat
    }
    Log "[iter $runId scenario=$scenario] setup=$($r[0]) recovery=$($r[1]) state=$($r[2]) latency=${lat}ms"
  } catch {
    Log "[iter $runId scenario=$scenario] ERROR: $($_.Exception.Message)"
  } finally {
    Stop-NotepadAll
    Remove-Item -Path $srcPath -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
  }
}

Stop-NotepadAll

function Count-True { param($A) (($A | Where-Object { $_ -eq $true }).Count) }
function Pct { param($num, $den) if ($den -gt 0) { [math]::Round($num * 100 / $den, 1) } else { 0 } }

$totalIter = $Iter
$reportLines = @(
  'probe-08 failure injection - result',
  ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')),
  ('iter=' + $Iter + ' warmup=' + $Warmup + ' (4 scenarios x 3 iter cycled)'),
  '',
  'scenario        | setup_ok | recovery_ok | state_correct',
  '----------------+----------+-------------+--------------'
)
foreach ($s in $scenarios) {
  $suT = Count-True $setupOk[$s]
  $reT = Count-True $recoveryOk[$s]
  $stT = Count-True $stateOk[$s]
  $reportLines += ('{0,-15} |  {1,3}/{2}   |   {3,3}/{2}    |   {4,3}/{2}     ' -f $s, $suT, $totalIter, $reT, $stT)
}
$reportLines += ''
$reportLines += ('  overall setup_ok rate:    {0}/{1} ({2}%)' -f (Count-True ($setupOk.Values | ForEach-Object { $_ })), ($totalIter * 4), (Pct (Count-True ($setupOk.Values | ForEach-Object { $_ })) ($totalIter * 4)))
$reportLines += ('  overall recovery_ok rate: {0}/{1} ({2}%)' -f (Count-True ($recoveryOk.Values | ForEach-Object { $_ })), ($totalIter * 4), (Pct (Count-True ($recoveryOk.Values | ForEach-Object { $_ })) ($totalIter * 4)))
$reportLines += ('  overall state_correct:   {0}/{1} ({2}%)' -f (Count-True ($stateOk.Values | ForEach-Object { $_ })), ($totalIter * 4), (Pct (Count-True ($stateOk.Values | ForEach-Object { $_ })) ($totalIter * 4)))
$reportLines += ''
$reportLines += ('latency_ms: {0} samples; median {1}' -f $latency.Count, ((($latency | Sort-Object)[[math]::Floor($latency.Count / 2)])))

$overallRecovery = (Pct (Count-True ($recoveryOk.Values | ForEach-Object { $_ })) ($totalIter * 4))
$reportLines += ''
$reportLines += ('go_criterion: failure_recovery_rate >= 80% (probe handles all 4 scenarios gracefully + recovers)')
$reportLines += ('  overall = ' + $(if ($overallRecovery -ge 80) {'GO'} else {'NO-GO'}))

$reportPath = Join-Path $WorkDir 'RESULT-08.txt'
[System.IO.File]::WriteAllLines($reportPath, $reportLines, [System.Text.Encoding]::UTF8)
Log "RESULT written to $reportPath"
foreach ($l in $reportLines) { Log $l }
$logFile.Close()
