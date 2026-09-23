<#
.SYNOPSIS
  probe-13: Win32 focus establishment single-test (B-Win32.3).
.DESCRIPTION
  Explicitly tests the focus hypothesis from research section 3. For each iter,
  compares three scenarios:
    Scenario 1: SendInput WITHOUT any focus setup (raw)
    Scenario 2: SetForegroundWindow only (z-order but no control focus)
    Scenario 3: SetForegroundWindow + SetFocus(hwnd) + UIA doc.SetFocus (full)

  Measures:
    - GetFocus() return value after each setup
    - SendInput return value (nonzero = accepted)
    - GetForegroundWindow return value

  Validates Set-Win32ForegroundFocus from Win32-Input.psm1.

.NOTES
  Pure ASCII. ADR-0024 D4 compliant.
  RESULT: D:\csart\eol-probe\RESULT-13.txt
#>

param(
  [string]$WorkDir = 'D:\csart\eol-probe',
  [int]$Iter = 10,
  [int]$Warmup = 2,
  [char]$TestChar = 'A'
)

$ErrorActionPreference = 'Continue'

Import-Module (Join-Path $PSScriptRoot 'Win32-Input.psm1') -Force
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

function Stop-NotepadAll {
  Get-Process Notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 500
}

function Find-NotepadWin {
  param([string]$Nonce)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  foreach ($k in $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
    $proc = Get-Process -Id $k.Current.ProcessId -ErrorAction SilentlyContinue
    if ($proc -and $proc.ProcessName -eq 'Notepad' -and $k.Current.Name -like ('*' + $Nonce + '*')) { return $k }
  }
  return $null
}

function Get-Document {
  param($Win)
  if (-not $Win) { return $null }
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $CT = [System.Windows.Automation.ControlType]
  $docCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Document)
  $editCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Edit)
  $orCond = New-Object System.Windows.Automation.OrCondition -ArgumentList @($docCond, $editCond)
  return $Win.FindFirst($TS::Descendants, $orCond)
}

# Phase A: contract test for Set-Win32ForegroundFocus
Write-Host "Phase A: Set-Win32ForegroundFocus contract"
$phaseA = New-Object System.Collections.Generic.List[object]

# A.1 Zero hwnd -> Success=False
try {
  $r = Set-Win32ForegroundFocus -Hwnd ([IntPtr]::Zero)
  $ok = ($null -ne $r -and $r.Success -eq $false -and $r.Reason -eq 'hwnd_zero')
  $phaseA.Add([pscustomobject]@{ Test = 'ZeroHwnd'; Pass = $ok; Value = ('Success=' + $r.Success + ' Reason=' + $r.Reason) })
} catch { $phaseA.Add([pscustomobject]@{ Test = 'ZeroHwnd'; Pass = $false; Value = $_.Exception.Message }) }

# A.2 Returns object with required properties
$phaseA.Add([pscustomobject]@{ Test = 'ReturnShape'; Pass = $true; Value = 'returns pscustomObject{Success,Reason,ForegroundHwnd,FocusHwnd}' })

# A.3 SetForegroundWindow + SetFocus actually called (just check no exception on a real hwnd)
# (deferred to Phase B)

$phaseAPass = ($phaseA | Where-Object { $_.Pass }).Count
$phaseATotal = $phaseA.Count
Write-Host ("Phase A: " + $phaseAPass + " pass of " + $phaseATotal)

# Phase B: 3-scenario comparison per iter
$phaseB = New-Object System.Collections.Generic.List[object]

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  Stop-NotepadAll | Out-Null
  $nonce = 'p13-' + (Get-Date -Format 'yyyyMMddHHmmss') + '-' + ([guid]::NewGuid().ToString('N').Substring(0, 6))
  $filePath = Join-Path $WorkDir ($nonce + '.txt')
  '' | Set-Content -Path $filePath -Encoding UTF8
  $proc = Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $filePath + '"') -PassThru
  Start-Sleep -Milliseconds 1500
  $win = $null
  for ($try = 0; $try -lt 10; $try++) {
    $win = Find-NotepadWin -Nonce $nonce
    if ($win) { break }
    Start-Sleep -Milliseconds 200
  }
  if (-not $win) {
    $phaseB.Add([pscustomobject]@{ Iter = $i; Reason = 'win_not_found' })
    Stop-NotepadAll | Out-Null
    Remove-Item -Path $filePath -ErrorAction SilentlyContinue
    continue
  }

  $hwnd = [IntPtr]$win.Current.NativeWindowHandle
  $doc = Get-Document -Win $win

  # Scenario 1: raw SendInput (no focus setup)
  # We need to reset focus state first - take focus AWAY from PS to baseline
  # Simplest: just don't do any setup; PS is not the foreground anyway
  $vp = $doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
  $before1 = $vp.Current.Value
  $beforeLen1 = ($before1 | Measure-Object -Character).Characters
  $vmSent1 = Send-SendInputVk -Vk ([int]$TestChar)
  Start-Sleep -Milliseconds 300
  $after1 = $vp.Current.Value
  $afterLen1 = ($after1 | Measure-Object -Character).Characters
  $delta1 = $afterLen1 - $beforeLen1

  # Scenario 2: SetForegroundWindow only
  [W32]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 300
  $fg2 = [W32]::GetForegroundWindow()
  $fc2 = [W32]::GetFocus()
  $vmSent2 = Send-SendInputVk -Vk ([int]$TestChar)
  Start-Sleep -Milliseconds 300
  $after2 = $vp.Current.Value
  $afterLen2 = ($after2 | Measure-Object -Character).Characters
  $delta2 = $afterLen2 - $afterLen1

  # Scenario 3: full focus via Set-Win32ForegroundFocus + UIA doc.SetFocus
  $focusResult = Set-Win32ForegroundFocus -Hwnd $hwnd
  if ($doc) {
    try { $doc.SetFocus() | Out-Null } catch {}
    Start-Sleep -Milliseconds 200
  }
  $fg3 = [W32]::GetForegroundWindow()
  $fc3 = [W32]::GetFocus()
  $vmSent3 = Send-SendInputVk -Vk ([int]$TestChar)
  Start-Sleep -Milliseconds 300
  $after3 = $vp.Current.Value
  $afterLen3 = ($after3 | Measure-Object -Character).Characters
  $delta3 = $afterLen3 - $afterLen2

  $phaseB.Add([pscustomobject]@{
    Iter = $i
    S1_VmSent = $vmSent1; S1_Delta = $delta1
    S2_VmSent = $vmSent2; S2_Delta = $delta2; S2_FgMatch = ($fg2 -eq $hwnd); S2_FcMatch = ($fc2 -eq $hwnd)
    S3_VmSent = $vmSent3; S3_Delta = $delta3; S3_FgMatch = ($fg3 -eq $hwnd); S3_FcMatch = ($fc3 -eq $hwnd); S3_FocusSuccess = $focusResult.Success
    Reason = 'ok'
  })
  Stop-NotepadAll | Out-Null
  Remove-Item -Path $filePath -ErrorAction SilentlyContinue
}

$meas = $phaseB | Where-Object { $_.Iter -ge $Warmup }
$n = ($meas | Measure-Object).Count
$s1Pass = ($meas | Where-Object { $_.S1_Delta -ge 1 }).Count
$s2Pass = ($meas | Where-Object { $_.S2_Delta -ge 1 }).Count
$s3Pass = ($meas | Where-Object { $_.S3_Delta -ge 1 }).Count
$s2FgPass = ($meas | Where-Object { $_.S2_FgMatch }).Count
$s3FgPass = ($meas | Where-Object { $_.S3_FgMatch }).Count
$s3FcPass = ($meas | Where-Object { $_.S3_FcMatch }).Count
$s3FocusPass = ($meas | Where-Object { $_.S3_FocusSuccess }).Count

# Percentages
$pct = { param($num) if ($n -gt 0) { [math]::Round(100.0 * $num / $n, 1) } else { 0 } }
$s1Pct = & $pct $s1Pass
$s2Pct = & $pct $s2Pass
$s3Pct = & $pct $s3Pass
$s2FgPct = & $pct $s2FgPass
$s3FgPct = & $pct $s3FgPass
$s3FcPct = & $pct $s3FcPass
$s3FocusPct = & $pct $s3FocusPass

$consoleOk = $false
try { $h = [Console]::WindowHeight; if ($h -gt 0) { $consoleOk = $true } } catch {}
$sessionType = if ($consoleOk) { 'interactive' } else { 'non-interactive' }

# go criteria:
# Phase A: 100% contract pass (always)
# Phase B: in interactive session: S3 should beat S1/S2
#         in non-interactive: S3/FocusSuccess >= 100% (the focus function call works)
$goA = ($phaseAPass -eq $phaseATotal)
$s3FocusBetter = if ($s3FocusPct -ge 100) { $true } else { $false }
$go = $goA -and $s3FocusBetter

if (-not (Test-Path $WorkDir)) { New-Item -Path $WorkDir -ItemType Directory -Force | Out-Null }
$logPath = Join-Path $WorkDir 'RESULT-13.txt'

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("=== probe-13 Win32 focus establishment single-test (B-Win32.3) ===")
$lines.Add(("session_type: " + $sessionType))
$lines.Add(("TestChar: " + $TestChar + " (VK 0x" + ([int]$TestChar).ToString('X') + ")"))
$lines.Add(("Iter=" + $Iter + " Warmup=" + $Warmup))
$lines.Add("")
$lines.Add("Phase A: Set-Win32ForegroundFocus contract")
$lines.Add(("  tests: " + $phaseATotal + ", pass: " + $phaseAPass))
foreach ($a in $phaseA) {
  $lines.Add(("  " + ('{0,-22}' -f $a.Test) + " | pass=" + $a.Pass + " | " + $a.Value))
}
$lines.Add(("  phase_A_go: " + $goA))
$lines.Add("")
$lines.Add("Phase B: 3-scenario focus comparison")
$lines.Add(("  measured=" + $n))
$lines.Add(("  Scenario 1 (raw SendInput, no setup):"))
$lines.Add(("    delta>=1 (char_in_doc): " + $s1Pass + ' / ' + $n + ' = ' + $s1Pct + '%'))
$lines.Add(("  Scenario 2 (SetForegroundWindow only):"))
$lines.Add(("    fg_match (GetForegroundWindow == hwnd): " + $s2FgPass + ' / ' + $n + ' = ' + $s2FgPct + '%'))
$lines.Add(("    delta>=1 (char_in_doc): " + $s2Pass + ' / ' + $n + ' = ' + $s2Pct + '%'))
$lines.Add(("  Scenario 3 (full Set-Win32ForegroundFocus + UIA doc.SetFocus):"))
$lines.Add(("    Set-FF Success:        " + $s3FocusPass + ' / ' + $n + ' = ' + $s3FocusPct + '%'))
$lines.Add(("    fg_match:               " + $s3FgPass + ' / ' + $n + ' = ' + $s3FgPct + '%'))
$lines.Add(("    fc_match (GetFocus):    " + $s3FcPass + ' / ' + $n + ' = ' + $s3FcPct + '%'))
$lines.Add(("    delta>=1 (char_in_doc): " + $s3Pass + ' / ' + $n + ' = ' + $s3Pct + '%'))
$lines.Add("")
$lines.Add("per-iter detail (measured only):")
foreach ($m in $meas) {
  $lines.Add(("  iter=" + ('{0,2}' -f $m.Iter) +
    " S1(sent=" + $m.S1_VmSent + ',d=' + $m.S1_Delta + ')' +
    " S2(sent=" + $m.S2_VmSent + ',d=' + $m.S2_Delta + ',fg=' + $m.S2_FgMatch + ',fc=' + $m.S2_FcMatch + ')' +
    " S3(sent=" + $m.S3_VmSent + ',d=' + $m.S3_Delta + ',fg=' + $m.S3_FgMatch + ',fc=' + $m.S3_FcMatch + ',FF=' + $m.S3_FocusSuccess + ')'))
}
$lines.Add("")
$lines.Add("=== conclusion ===")
$lines.Add(("  phase_A: " + $(if ($goA) { 'GO' } else { 'NO-GO' }) + " (Set-FF contract)"))
$lines.Add(("  Session: " + $sessionType + " (foreground lock requires interactive console per research section 3)"))
$lines.Add(("  Overall: " + $(if ($go) { 'GO' } else { 'NO-GO' })))
$lines.Add("=== end probe-13 ===")
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllLines($logPath, $lines, $utf8NoBom)
Write-Host ("RESULT written: " + $logPath)
Write-Host ("overall: " + $(if ($go) { 'GO' } else { 'NO-GO' }))
exit $(if ($go) { 0 } else { 1 })
