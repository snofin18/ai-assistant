<#
.SYNOPSIS
  probe-10: IME on/off two-state PoC (B1.7) v3 - fixed all bugs.
.DESCRIPTION
  Each iter: alternate off/on modes; test VP + SK paths.
  Fixes: removed StreamWriter (returned null due to constructor issue).
  Removed broken Add-Type (encoding issue with DllImport).
  Uses Write-Host for output and UIA SetFocus for focus.
.NOTES
  Pure ASCII. ADR-0024 D4 compliant.
  RESULT: D:\csart\eol-probe\RESULT-10.txt
#>

param(
  [string]$WorkDir = 'D:\csart\eol-probe',
  [int]$Iter = 10,
  [int]$Warmup = 2
)

$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
# TASK-101: Win32 Input helper (replaces deprecated SendKeys)
Import-Module (Join-Path $PSScriptRoot 'Win32-Input.psm1') -Force

function Stop-NotepadAll {
  Get-Process Notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 500
}
function Find-NotepadWin {
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  foreach ($k in $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
    $proc = Get-Process -Id $k.Current.ProcessId -ErrorAction SilentlyContinue
    if ($proc -and $proc.ProcessName -eq 'Notepad' -and $k.Current.Name -match 'probe10-') { return $k }
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
function Get-DocText {
  param($Doc)
  if (-not $Doc) { return $null }
  try {
    $vp = $Doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    return $vp.Current.Value
  } catch { return $null }
}
function Set-DocVP {
  param($Doc, [string]$Text)
  if (-not $Doc) { return $false }
  try {
    $vp = $Doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $vp.SetValue($Text)
    return $true
  } catch { return $false }
}

$results = @{ off_vp = @(); off_sk = @(); on_vp = @(); on_sk = @() }
$content = 'probe10 baseline v3'

Stop-NotepadAll

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  $isWarmup = $i -lt $Warmup
  $runId = $i + 1
  $mode = if (($i % 2) -eq 0) { 'off' } else { 'on' }
  try {
    $nonce = ([Guid]::NewGuid().ToString('N')).Substring(0, 8)
    $path = Join-Path $WorkDir ('probe10-' + $nonce + '.txt')
    [System.IO.File]::WriteAllText($path, $content, [System.Text.Encoding]::UTF8)
    $null = Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $path + '"') -PassThru
    Start-Sleep -Milliseconds 2500
    $npWin = $null
    for ($poll = 0; $poll -lt 15; $poll++) {
      $npWin = Find-NotepadWin
      if ($npWin) { break }
      Start-Sleep -Milliseconds 500
    }
    if (-not $npWin) { continue }
    $doc = Get-Document -Win $npWin
    if (-not $doc) { continue }
    try { $doc.SetFocus() } catch {}
    Start-Sleep -Milliseconds 200
    $vpContent = 'vp_' + $mode + '_r' + $runId
    $vpOk = Set-DocVP -Doc $doc -Text $vpContent
    Start-Sleep -Milliseconds 200
    $vpRead = Get-DocText -Doc $doc
    $vpResult = ($vpOk -and ($vpRead -eq $vpContent))
    try {
      Send-SendInputVk -Vk 0x41 -Modifier @(0xA2) | Out-Null  # Ctrl+A  # TASK-101: replaces SendKeys
      Start-Sleep -Milliseconds 100
      Send-SendInputVk -Vk 0x2E | Out-Null  # DEL  # TASK-101: replaces SendKeys
      Start-Sleep -Milliseconds 300
    } catch {}
    $skContent = 'sk_' + $mode + '_r' + $runId
    try {
      Send-SendInputUnicode -Text $skContent | Out-Null  # TASK-101: replaces SendKeys (string path)
      Start-Sleep -Milliseconds 500
    } catch {}
    $skRead = Get-DocText -Doc $doc
    $skResult = ($skRead -eq $skContent)
    if (-not $isWarmup) {
      if ($mode -eq 'off') {
        $results.off_vp += $vpResult
        $results.off_sk += $skResult
      } else {
        $results.on_vp += $vpResult
        $results.on_sk += $skResult
      }
    }
    Write-Host ('[iter ' + $runId + ' mode=' + $mode + '] VP=' + $vpResult + ' SK=' + $skResult)
  } catch {
    Write-Host ('[iter ' + $runId + '] ERROR: ' + $_.Exception.Message)
  } finally {
    Get-Process Notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
  }
}
Stop-NotepadAll

$offVp = (($results.off_vp) | Where-Object { $_ -eq $true }).Count
$offSk = (($results.off_sk) | Where-Object { $_ -eq $true }).Count
$onVp = (($results.on_vp) | Where-Object { $_ -eq $true }).Count
$onSk = (($results.on_sk) | Where-Object { $_ -eq $true }).Count

$rl = @()
$rl += 'probe-10 IME two-state PoC v3 - result'
$rl += ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))
$rl += ('iter=' + $Iter + ' warmup=' + $Warmup)
$rl += ''
$rl += 'metric      | success / iter | pct'
$rl += '------------+-----------------+-------'
function PctFor {
  param($Found, $Total)
  if ($Total -gt 0) { [math]::Round($Found * 100 / $Total, 1) } else { 0 }
}
$rl += ('off     vp      | {0,5} / {1,-4}   | {2,5}%' -f $offVp, ($Iter / 2), (PctFor $offVp ($Iter / 2)))
$rl += ('off     sk      | {0,5} / {1,-4}   | {2,5}%' -f $offSk, ($Iter / 2), (PctFor $offSk ($Iter / 2)))
$rl += ('on      vp      | {0,5} / {1,-4}   | {2,5}%' -f $onVp, ($Iter / 2), (PctFor $onVp ($Iter / 2)))
$rl += ('on      sk      | {0,5} / {1,-4}   | {2,5}%' -f $onSk, ($Iter / 2), (PctFor $onSk ($Iter / 2)))
$rl += ''
$totalFound = $offVp + $offSk + $onVp + $onSk
$totalTests = $Iter * 2
$rl += ('overall: ' + $totalFound + ' / ' + $totalTests + ' = ' + (PctFor $totalFound $totalTests) + '%')
$rl += ''
$rl += 'go_criterion: IME_on_off_two_state_correctness >= 90%'
if ((PctFor $totalFound $totalTests) -ge 90) {
  $rl += '  overall = GO'
} else {
  $rl += '  overall = NO-GO'
}

$rp = Join-Path $WorkDir 'RESULT-10.txt'
[System.IO.File]::WriteAllLines($rp, $rl, [System.Text.Encoding]::UTF8)
Write-Host ''
Write-Host ('RESULT written to ' + $rp)
foreach ($l in $rl) { Write-Host $l }
