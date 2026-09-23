<#
.SYNOPSIS
  probe-12: Win32 SendInput Unicode path single-test (B-Win32.2).
.DESCRIPTION
  Validates Win32-Input.psm1's Send-SendInputUnicode function using CJK and mixed
  Unicode strings. Three phases (same structure as probe-11):

  Phase A: module contract - exercise Send-SendInputUnicode with various inputs
  Phase B: best-effort delivery - send CJK string via SendInput; verify in UIA
  Phase C: UIA SetValue baseline - control for document manipulation

  CJK strings are constructed at runtime via [char]0x... per ADR-0024 D4.

.NOTES
  Pure ASCII. ADR-0024 D4 compliant.
  RESULT: D:\csart\eol-probe\RESULT-12.txt
#>

param(
  [string]$WorkDir = 'D:\csart\eol-probe',
  [int]$Iter = 10,
  [int]$Warmup = 2
)

$ErrorActionPreference = 'Continue'

Import-Module (Join-Path $PSScriptRoot 'Win32-Input.psm1') -Force
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

# Construct CJK test strings at runtime (ADR-0024 D4 compliance)
# "ZhongWenZhong" in Chinese: [char]0x4E2D (Zhong) + [char]0x6587 (Wen) + [char]0x4E2D (Zhong)
$CN_TEST = [char]0x4E2D + [char]0x6587 + [char]0x4E2D
# Mixed ASCII + CJK + symbol: "Hi" + [char]0x4F60 + [char]0x597D + "!"
$MIX_TEST = "Hi" + [char]0x4F60 + [char]0x597D + "!"
# Single CJK char for delta=1 verification: [char]0x535A
$SINGLE_CN = [char]0x535A

# ========== Phase A: module contract ==========

$phaseA = New-Object System.Collections.Generic.List[object]

# A.1 Send-SendInputUnicode with ASCII
try {
  $r = Send-SendInputUnicode -Text "hello"
  if ($r -is [uint32] -or $r -is [int]) { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_ASCII'; Pass = $true; Value = $r }) }
  else { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_ASCII'; Pass = $false; Value = "wrong type" }) }
} catch { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_ASCII'; Pass = $false; Value = $_.Exception.Message }) }

# A.2 Send-SendInputUnicode with CJK (single char)
try {
  $r = Send-SendInputUnicode -Text $SINGLE_CN
  if ($r -is [uint32] -or $r -is [int]) { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_SingleCJK'; Pass = $true; Value = $r }) }
  else { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_SingleCJK'; Pass = $false; Value = "wrong type" }) }
} catch { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_SingleCJK'; Pass = $false; Value = $_.Exception.Message }) }

# A.3 Send-SendInputUnicode with CJK (multi)
try {
  $r = Send-SendInputUnicode -Text $CN_TEST
  if ($r -is [uint32] -or $r -is [int]) { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_MultiCJK'; Pass = $true; Value = $r }) }
  else { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_MultiCJK'; Pass = $false; Value = "wrong type" }) }
} catch { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_MultiCJK'; Pass = $false; Value = $_.Exception.Message }) }

# A.4 Send-SendInputUnicode with mixed
try {
  $r = Send-SendInputUnicode -Text $MIX_TEST
  if ($r -is [uint32] -or $r -is [int]) { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_Mixed'; Pass = $true; Value = $r }) }
  else { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_Mixed'; Pass = $false; Value = "wrong type" }) }
} catch { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_Mixed'; Pass = $false; Value = $_.Exception.Message }) }

# A.5 Verify Send-SendInputUnicode empty returns 0
try {
  $r = Send-SendInputUnicode -Text ""
  if ($r -eq 0) { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_Empty'; Pass = $true; Value = $r }) }
  else { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_Empty'; Pass = $false; Value = ("got " + $r) }) }
} catch { $phaseA.Add([pscustomobject]@{ Test = 'Unicode_Empty'; Pass = $false; Value = $_.Exception.Message }) }

$phaseAPass = ($phaseA | Where-Object { $_.Pass }).Count
$phaseATotal = $phaseA.Count
Write-Host ("Phase A: " + $phaseAPass + " pass of " + $phaseATotal)

# ========== Phase B + C: best-effort + control ==========

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

$phaseB = New-Object System.Collections.Generic.List[object]
$phaseC = New-Object System.Collections.Generic.List[object]

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  Stop-NotepadAll | Out-Null

  # Phase B: Unicode delivery
  $nonce = 'p12-' + (Get-Date -Format 'yyyyMMddHHmmss') + '-' + ([guid]::NewGuid().ToString('N').Substring(0, 6))
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
    $phaseB.Add([pscustomobject]@{ Iter = $i; VmSent = 0; CharInDoc = $false; Reason = 'win_not_found' })
  } else {
    $hwnd = [IntPtr]$win.Current.NativeWindowHandle
    $focusResult = Set-Win32ForegroundFocus -Hwnd $hwnd
    $doc = Get-Document -Win $win
    if (-not $doc) {
      $phaseB.Add([pscustomobject]@{ Iter = $i; VmSent = 0; CharInDoc = $false; Reason = 'doc_not_found' })
    } else {
      try { $doc.SetFocus() | Out-Null } catch {}
      Start-Sleep -Milliseconds 200
      $vp = $doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
      $before = $vp.Current.Value
      $beforeLen = ($before | Measure-Object -Character).Characters
      $vmSent = Send-SendInputUnicode -Text $SINGLE_CN
      Start-Sleep -Milliseconds 500
      $after = $vp.Current.Value
      $afterLen = ($after | Measure-Object -Character).Characters
      $lenDelta = $afterLen - $beforeLen
      # Check if CJK char is in delta region
      $charInDoc = ($after.IndexOf($SINGLE_CN) -ge $beforeLen)
      $phaseB.Add([pscustomobject]@{
        Iter = $i; VmSent = $vmSent; FocusOk = $focusResult.Success
        CharInDoc = $charInDoc; DocLenBefore = $beforeLen; DocLenAfter = $afterLen
        LenDelta = $lenDelta; Reason = if (-not $focusResult.Success) { 'focus_fail' } elseif (-not $charInDoc) { 'char_missing' } else { 'ok' }
      })
    }
  }
  Stop-NotepadAll | Out-Null
  Remove-Item -Path $filePath -ErrorAction SilentlyContinue

  # Phase C: UIA SetValue baseline with CJK
  $nonceC = 'p12c-' + (Get-Date -Format 'yyyyMMddHHmmss') + '-' + ([guid]::NewGuid().ToString('N').Substring(0, 6))
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
    $phaseC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $false; Reason = 'win_not_found' })
  } else {
    $docC = Get-Document -Win $winC
    if (-not $docC) {
      $phaseC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $false; Reason = 'doc_not_found' })
    } else {
      try {
        $vpC = $docC.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $vpC.SetValue($CN_TEST)
        Start-Sleep -Milliseconds 200
        $vpC2 = $docC.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $readBack = $vpC2.Current.Value
        $ok = ($readBack -eq $CN_TEST)
        $phaseC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $ok; ReadBack = $readBack; Reason = if ($ok) { 'ok' } else { 'setvalue_mismatch' } })
      } catch { $phaseC.Add([pscustomobject]@{ Iter = $i; UiSetValueOk = $false; Reason = 'exception:' + $_.Exception.Message }) }
    }
  }
  Stop-NotepadAll | Out-Null
  Remove-Item -Path $filePathC -ErrorAction SilentlyContinue
}

$measB = $phaseB | Where-Object { $_.Iter -ge $Warmup }
$nB = ($measB | Measure-Object).Count
$charPassB = ($measB | Where-Object { $_.CharInDoc }).Count
$charPctB = if ($nB -gt 0) { [math]::Round(100.0 * $charPassB / $nB, 1) } else { 0 }
$sentNonZeroB = ($measB | Where-Object { $_.VmSent -gt 0 }).Count
$sentPctB = if ($nB -gt 0) { [math]::Round(100.0 * $sentNonZeroB / $nB, 1) } else { 0 }

$measC = $phaseC | Where-Object { $_.Iter -ge $Warmup }
$nC = ($measC | Measure-Object).Count
$setValueOkC = ($measC | Where-Object { $_.UiSetValueOk }).Count
$setValuePctC = if ($nC -gt 0) { [math]::Round(100.0 * $setValueOkC / $nC, 1) } else { 0 }

$consoleOk = $false
try { $h = [Console]::WindowHeight; if ($h -gt 0) { $consoleOk = $true } } catch {}
$sessionType = if ($consoleOk) { 'interactive (console attached)' } else { 'non-interactive (no console)' }

$goA = ($phaseAPass -eq $phaseATotal)
$goB = ($charPctB -ge 100)
$goC = ($setValuePctC -ge 100)
$sessionOk = if ($consoleOk) { $goB } else { $true }
$go = $goA -and $goC -and $sessionOk

if (-not (Test-Path $WorkDir)) { New-Item -Path $WorkDir -ItemType Directory -Force | Out-Null }
$logPath = Join-Path $WorkDir 'RESULT-12.txt'

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("=== probe-12 Win32 SendInput Unicode single-test (B-Win32.2) ===")
$lines.Add(("session_type: " + $sessionType))
# hex of test single CJK char: U+535A (literal)
$lines.Add(("Test string (CJK setvalue): " + $CN_TEST))
$lines.Add(("Test string (mixed): " + $MIX_TEST))
$lines.Add(("Iter=" + $Iter + " Warmup=" + $Warmup))
$lines.Add("")
$lines.Add("Phase A: module contract")
$lines.Add(("  tests: " + $phaseATotal + ", pass: " + $phaseAPass))
foreach ($a in $phaseA) {
  $lines.Add(("  " + ('{0,-22}' -f $a.Test) + " | pass=" + $a.Pass + " | " + $a.Value))
}
$lines.Add(("  phase_A_go: " + $goA))
$lines.Add("")
$lines.Add("Phase B: best-effort CJK delivery")
$lines.Add(("  measured=" + $nB))
$lines.Add(("  char_in_doc:        " + ('{0,4}' -f $charPassB) + ' / ' + $nB + ' = ' + $charPctB + '%'))
$lines.Add(("  sendinput_return_nz: " + ('{0,4}' -f $sentNonZeroB) + ' / ' + $nB + ' = ' + $sentPctB + '% (informational)'))
$lines.Add(("  phase_B_go: " + $goB))
foreach ($m in $measB) {
  $lines.Add(("  iter=" + ('{0,2}' -f $m.Iter) +
    " vmSent=" + ('{0,3}' -f $m.VmSent) +
    " delta=" + ('{0,3}' -f $m.LenDelta) +
    " charInDoc=" + $m.CharInDoc +
    " reason=" + $m.Reason))
}
$lines.Add("")
$lines.Add("Phase C: UIA SetValue baseline (CJK control)")
$lines.Add(("  measured=" + $nC))
$lines.Add(("  uia_setvalue_ok: " + ('{0,4}' -f $setValueOkC) + ' / ' + $nC + ' = ' + $setValuePctC + '%'))
$lines.Add(("  phase_C_go: " + $goC))
foreach ($m in $measC) {
  $lines.Add(("  iter=" + ('{0,2}' -f $m.Iter) + " uiSetValueOk=" + $m.UiSetValueOk + " reason=" + $m.Reason))
}
$lines.Add("")
$lines.Add("=== conclusion ===")
$lines.Add(("  phase_A: " + $(if ($goA) { 'GO' } else { 'NO-GO' }) + " (module contract)"))
$lines.Add(("  phase_B: " + $(if ($goB) { 'GO' } else { 'NO-GO' }) + " (CJK delivery) - " + $sessionType))
$lines.Add(("  phase_C: " + $(if ($goC) { 'GO' } else { 'NO-GO' }) + " (UIA SetValue CJK baseline)"))
$lines.Add(("  overall: " + $(if ($go) { 'GO' } else { 'NO-GO' })))
$lines.Add("=== end probe-12 ===")
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllLines($logPath, $lines, $utf8NoBom)
Write-Host ("RESULT written: " + $logPath)
Write-Host ("overall: " + $(if ($go) { 'GO' } else { 'NO-GO' }))
exit $(if ($go) { 0 } else { 1 })
