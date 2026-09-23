<#
.SYNOPSIS
  probe-11: Win32 SendInput VK path single-test (B-Win32.1).

.DESCRIPTION
  Validates the Win32-Input.psm1 module's Send-SendInputVk function. Three phases:

  Phase A (module contract, always runs):
    - Import Win32-Input.psm1
    - Call each exported function with sample inputs
    - Confirm no exception, returns expected type (uint32)
    - go criterion: 100% no-throw

  Phase B (best-effort delivery, environment-dependent):
    - Open Notepad with a unique file
    - Try Set-Win32ForegroundFocus + Send-SendInputVk(VK_Z)
    - Verify character in UIA ValuePattern.Current.Value
    - go criterion (only meaningful in interactive session): 100% char_in_doc

  Phase C (control - UIA SetValue baseline):
    - Open Notepad with a unique file
    - UIA ValuePattern.SetValue("UIA control baseline")
    - Verify value actually set
    - go criterion: 100% uia_setvalue_ok

  10 + 2 warmup iterations.

.NOTES
  Pure ASCII. ADR-0024 D4 compliant.
  RESULT: D:\csart\eol-probe\RESULT-11.txt
  Imports Win32-Input.psm1 from this folder.
#>

param(
  [string]$WorkDir = 'D:\csart\eol-probe',
  [int]$Iter = 10,
  [int]$Warmup = 2,
  [char]$TestChar = 'Z'
)

$ErrorActionPreference = 'Continue'

# Load Win32-Input module from same folder.
Import-Module (Join-Path $PSScriptRoot 'Win32-Input.psm1') -Force

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

# ========== Phase A: module contract (always runs) ==========

Write-Host "Phase A: module contract"

$phaseAContract = New-Object System.Collections.Generic.List[object]

# A.1 Send-SendInputVk returns uint32
try {
  $r = Send-SendInputVk -Vk 0x5A
  if ($r -is [uint32] -or $r -is [int]) { $phaseAContract.Add([pscustomobject]@{ Test = 'SendVk_NoMod'; Pass = $true; Value = $r }) }
  else { $phaseAContract.Add([pscustomobject]@{ Test = 'SendVk_NoMod'; Pass = $false; Value = "type=$($r.GetType().FullName)" }) }
} catch { $phaseAContract.Add([pscustomobject]@{ Test = 'SendVk_NoMod'; Pass = $false; Value = $_.Exception.Message }) }

# A.2 Send-SendInputVk with modifier
try {
  $r = Send-SendInputVk -Vk 0x41 -Modifier @(0xA2)  # Ctrl+A
  if ($r -is [uint32] -or $r -is [int]) { $phaseAContract.Add([pscustomobject]@{ Test = 'SendVk_WithMod'; Pass = $true; Value = $r }) }
  else { $phaseAContract.Add([pscustomobject]@{ Test = 'SendVk_WithMod'; Pass = $false; Value = "type=$($r.GetType().FullName)" }) }
} catch { $phaseAContract.Add([pscustomobject]@{ Test = 'SendVk_WithMod'; Pass = $false; Value = $_.Exception.Message }) }

# A.3 Send-SendInputUnicode
try {
  $r = Send-SendInputUnicode -Text "abc"
  if ($r -is [uint32] -or $r -is [int]) { $phaseAContract.Add([pscustomobject]@{ Test = 'SendUnicode'; Pass = $true; Value = $r }) }
  else { $phaseAContract.Add([pscustomobject]@{ Test = 'SendUnicode'; Pass = $false; Value = "type=$($r.GetType().FullName)" }) }
} catch { $phaseAContract.Add([pscustomobject]@{ Test = 'SendUnicode'; Pass = $false; Value = $_.Exception.Message }) }

# A.4 Send-SendInputUnicode with empty
try {
  $r = Send-SendInputUnicode -Text ""
  if ($r -is [uint32] -or $r -is [int]) { $phaseAContract.Add([pscustomobject]@{ Test = 'SendUnicode_Empty'; Pass = $true; Value = $r }) }
  else { $phaseAContract.Add([pscustomobject]@{ Test = 'SendUnicode_Empty'; Pass = $false; Value = "type=$($r.GetType().FullName)" }) }
} catch { $phaseAContract.Add([pscustomobject]@{ Test = 'SendUnicode_Empty'; Pass = $false; Value = $_.Exception.Message }) }

# A.5 Set-Win32ForegroundFocus on zero hwnd returns object
try {
  $r = Set-Win32ForegroundFocus -Hwnd ([IntPtr]::Zero)
  if ($null -ne $r -and $r.PSObject.Properties['Success']) { $phaseAContract.Add([pscustomobject]@{ Test = 'SetFF_Zero'; Pass = $true; Value = ("Success=" + $r.Success) }) }
  else { $phaseAContract.Add([pscustomobject]@{ Test = 'SetFF_Zero'; Pass = $false; Value = "no result object" }) }
} catch { $phaseAContract.Add([pscustomobject]@{ Test = 'SetFF_Zero'; Pass = $false; Value = $_.Exception.Message }) }

# A.6 Get-VkFromChar
$vkTests = @(
  @{ CH = 'A'; Expected = 0x41 },
  @{ CH = 'a'; Expected = 0x41 },
  @{ CH = 'Z'; Expected = 0x5A },
  @{ CH = '5'; Expected = 0x35 }
)
foreach ($v in $vkTests) {
  try {
    $r = Get-VkFromChar -Char ([char]$v.CH)
    if ($r -eq $v.Expected) { $phaseAContract.Add([pscustomobject]@{ Test = ('VkFromChar_' + $v.CH); Pass = $true; Value = $r }) }
    else { $phaseAContract.Add([pscustomobject]@{ Test = ('VkFromChar_' + $v.CH); Pass = $false; Value = ("got " + $r + " expected " + $v.Expected) }) }
  } catch { $phaseAContract.Add([pscustomobject]@{ Test = ('VkFromChar_' + $v.CH); Pass = $false; Value = $_.Exception.Message }) }
}

$phaseAPass = ($phaseAContract | Where-Object { $_.Pass }).Count
$phaseAFail = ($phaseAContract | Where-Object { -not $_.Pass }).Count
$phaseATotal = $phaseAContract.Count
Write-Host ("Phase A: " + $phaseAPass + " pass / " + $phaseAFail + " fail of " + $phaseATotal)

# ========== Phase B: best-effort delivery (session-dependent) ==========

Write-Host "Phase B: best-effort delivery"

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

# Probe B: best-effort delivery + capture environment reality
$phaseBRows = New-Object System.Collections.Generic.List[object]
$phaseBRowsC = New-Object System.Collections.Generic.List[object]  # control (UIA SetValue) baseline

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  Stop-NotepadAll | Out-Null

  $nonce = 'p11-' + (Get-Date -Format 'yyyyMMddHHmmss') + '-' + ([guid]::NewGuid().ToString('N').Substring(0, 6))
  $filePath = Join-Path $WorkDir ($nonce + '.txt')
  '' | Set-Content -Path $filePath -Encoding UTF8

  # Phase B: SendInput delivery attempt
  $proc = Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $filePath + '"') -PassThru
  Start-Sleep -Milliseconds 1500
  $win = $null
  for ($try = 0; $try -lt 10; $try++) {
    $win = Find-NotepadWin -Nonce $nonce
    if ($win) { break }
    Start-Sleep -Milliseconds 200
  }
  if (-not $win) {
    $phaseBRows.Add([pscustomobject]@{ Iter = $i; VmSent = 0; FocusOk = $false; CharInDoc = $false; Reason = 'win_not_found' })
  } else {
    $hwnd = [IntPtr]$win.Current.NativeWindowHandle
    $focusResult = Set-Win32ForegroundFocus -Hwnd $hwnd
    $doc = Get-Document -Win $win
    if (-not $doc) {
      $phaseBRows.Add([pscustomobject]@{ Iter = $i; VmSent = 0; FocusOk = $focusResult.Success; CharInDoc = $false; Reason = 'doc_not_found' })
    } else {
      try { $doc.SetFocus() | Out-Null } catch {}
      Start-Sleep -Milliseconds 200
      $vp = $doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
      $before = $vp.Current.Value
      $beforeLen = ($before | Measure-Object -Character).Characters
      $vk = [int]$TestChar
      $vmSent = Send-SendInputVk -Vk $vk
      Start-Sleep -Milliseconds 500
      $after = $vp.Current.Value
      $afterLen = ($after | Measure-Object -Character).Characters
      $lenDelta = $afterLen - $beforeLen
      $charInDoc = ($after.IndexOf($TestChar) -ge $beforeLen)
      $phaseBRows.Add([pscustomobject]@{
        Iter = $i; VmSent = $vmSent; FocusOk = $focusResult.Success
        CharInDoc = $charInDoc; DocLenBefore = $beforeLen; DocLenAfter = $afterLen
        LenDelta = $lenDelta; Reason = if (-not $focusResult.Success) { 'focus_fail' } elseif (-not $charInDoc) { 'char_missing' } else { 'ok' }
      })
    }
  }
  Stop-NotepadAll | Out-Null
  Remove-Item -Path $filePath -ErrorAction SilentlyContinue

  # Phase C: control - UIA SetValue baseline (independent)
  $nonceC = 'p11c-' + (Get-Date -Format 'yyyyMMddHHmmss') + '-' + ([guid]::NewGuid().ToString('N').Substring(0, 6))
  $filePathC = Join-Path $WorkDir ($nonceC + '.txt')
  '' | Set-Content -Path $filePathC -Encoding UTF8
  $procC = Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $filePathC + '"') -PassThru
  Start-Sleep -Milliseconds 1500
  $winC = $null
  for ($try = 0; $try -lt 10; $try++) {
    $winC = Find-NotepadWin -Nonce $nonceC
    if ($winC) { break }
    Start-Sleep -Milliseconds 200
  }
  if (-not $winC) {
    $phaseBRowsC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $false; Reason = 'win_not_found' })
  } else {
    $docC = Get-Document -Win $winC
    if (-not $docC) {
      $phaseBRowsC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $false; Reason = 'doc_not_found' })
    } else {
      try {
        $vpC = $docC.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $vpC.SetValue('UIA control baseline')
        Start-Sleep -Milliseconds 200
        $vpC2 = $docC.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $readBack = $vpC2.Current.Value
        $ok = ($readBack -eq 'UIA control baseline')
        $phaseBRowsC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $ok; ReadBack = $readBack; Reason = if ($ok) { 'ok' } else { 'setvalue_mismatch' } })
      } catch { $phaseBRowsC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $false; Reason = 'exception:' + $_.Exception.Message }) }
    }
  }
  Stop-NotepadAll | Out-Null
  Remove-Item -Path $filePathC -ErrorAction SilentlyContinue
}

$warmB = $phaseBRows | Where-Object { $_.Iter -lt $Warmup }
$measB = $phaseBRows | Where-Object { $_.Iter -ge $Warmup }
$nB = ($measB | Measure-Object).Count
$focusPassB = ($measB | Where-Object { $_.FocusOk }).Count
$charPassB = ($measB | Where-Object { $_.CharInDoc }).Count
$sentNonZeroB = ($measB | Where-Object { $_.VmSent -gt 0 }).Count
$focusPctB = if ($nB -gt 0) { [math]::Round(100.0 * $focusPassB / $nB, 1) } else { 0 }
$charPctB = if ($nB -gt 0) { [math]::Round(100.0 * $charPassB / $nB, 1) } else { 0 }
$sentPctB = if ($nB -gt 0) { [math]::Round(100.0 * $sentNonZeroB / $nB, 1) } else { 0 }

$warmC = $phaseBRowsC | Where-Object { $_.Iter -lt $Warmup }
$measC = $phaseBRowsC | Where-Object { $_.Iter -ge $Warmup }
$nC = ($measC | Measure-Object).Count
$setValueOkC = ($measC | Where-Object { $_.UiSetValueOk }).Count
$setValuePctC = if ($nC -gt 0) { [math]::Round(100.0 * $setValueOkC / $nC, 1) } else { 0 }

# Determine session type
$consoleOk = $false
try { $h = [Console]::WindowHeight; if ($h -gt 0) { $consoleOk = $true } } catch {}
$sessionType = if ($consoleOk) { "interactive (console attached)" } else { "non-interactive (no console)" }

# go criteria:
# Phase A: 100% contract pass (module API works)
# Phase B: focus_ok >= 100% AND char_in_doc >= 100% (in interactive session); NO-GO in non-interactive is EXPECTED
# Phase C: UIA SetValue >= 100% (control - should work regardless of session)
$goA = ($phaseAPass -eq $phaseATotal)
$goB = ($focusPctB -ge 100) -and ($charPctB -ge 100)
$goC = ($setValuePctC -ge 100)
# Overall go: A always, B only in interactive, C always (control)
$sessionOk = if ($consoleOk) { $goB } else { $true }; $go = $goA -and $goC -and $sessionOk

# Ensure output dir
if (-not (Test-Path $WorkDir)) { New-Item -Path $WorkDir -ItemType Directory -Force | Out-Null }
$logPath = Join-Path $WorkDir 'RESULT-11.txt'

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("=== probe-11 Win32 SendInput VK single-test (B-Win32.1) ===")
$lines.Add(("session_type: " + $sessionType))
$lines.Add(("TestChar: " + $TestChar + " (VK 0x" + ([int]$TestChar).ToString('X') + ")"))
$lines.Add(("Iter=" + $Iter + " Warmup=" + $Warmup))
$lines.Add("")
$lines.Add("Phase A: module contract (always runs)")
$lines.Add(("  tests: " + $phaseATotal + ", pass: " + $phaseAPass + ", fail: " + $phaseAFail))
foreach ($a in $phaseAContract) {
  $lines.Add(("  " + ('{0,-22}' -f $a.Test) + " | pass=" + $a.Pass + " | " + $a.Value))
}
$lines.Add(("  phase_A_go: " + $goA))
$lines.Add("")
$lines.Add("Phase B: best-effort delivery (session-dependent)")
$lines.Add(("  measured=" + $nB))
$lines.Add(("  focus_ok:       " + ('{0,4}' -f $focusPassB) + ' / ' + $nB + ' = ' + $focusPctB + '%'))
$lines.Add(("  char_in_doc:    " + ('{0,4}' -f $charPassB) + ' / ' + $nB + ' = ' + $charPctB + '%'))
$lines.Add(("  sendinput_return_nonzero: " + ('{0,4}' -f $sentNonZeroB) + ' / ' + $nB + ' = ' + $sentPctB + '% (informational; 0 = thread had no focus to receive)'))
$lines.Add(("  phase_B_go (interactive only): " + $goB))
foreach ($m in $measB) {
  $lines.Add(("  iter=" + ('{0,2}' -f $m.Iter) +
    " vmSent=" + ('{0,3}' -f $m.VmSent) +
    " focusOk=" + $m.FocusOk +
    " delta=" + ('{0,3}' -f $m.LenDelta) +
    " charInDoc=" + $m.CharInDoc +
    " reason=" + $m.Reason))
}
$lines.Add("")
$lines.Add("Phase C: UIA SetValue control baseline (independent of input)")
$lines.Add(("  measured=" + $nC))
$lines.Add(("  uia_setvalue_ok: " + ('{0,4}' -f $setValueOkC) + ' / ' + $nC + ' = ' + $setValuePctC + '%'))
$lines.Add(("  phase_C_go: " + $goC))
foreach ($m in $measC) {
  $lines.Add(("  iter=" + ('{0,2}' -f $m.Iter) + " uiSetValueOk=" + $m.UiSetValueOk + " reason=" + $m.Reason))
}
$lines.Add("")
$lines.Add("=== conclusion ===")
$lines.Add(("  phase_A: " + $(if ($goA) { 'GO' } else { 'NO-GO' }) + " (module contract)"))
$lines.Add(("  phase_B: " + $(if ($goB) { 'GO' } else { 'NO-GO' }) + " (delivery) - " + $(if ($consoleOk) { 'in interactive session' } else { 'in non-interactive session (expected NO-GO per research section 3)' })))
$lines.Add(("  phase_C: " + $(if ($goC) { 'GO' } else { 'NO-GO' }) + " (UIA control baseline)"))
$lines.Add(("  overall: " + $(if ($go) { 'GO' } else { 'NO-GO' })))
$lines.Add("")
$lines.Add("=== end probe-11 ===")
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllLines($logPath, $lines, $utf8NoBom)
Write-Host ("RESULT written: " + $logPath)
Write-Host ("overall: " + $(if ($go) { 'GO' } else { 'NO-GO' }))
exit $(if ($go) { 0 } else { 1 })
