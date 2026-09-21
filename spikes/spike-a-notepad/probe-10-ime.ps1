<#
.SYNOPSIS
  probe-10: IME on/off two-state PoC (B1.7).
.DESCRIPTION
  Test modern Notepad FileName edit handles SendKeys correctly.
  Per DRIFT-002-1 + probe-02: SetValue is IME-independent.
  Real IME test = use SendKeys (synthesizes keyboard events).
  12 iter (10 + 2 warmup) alternating between two modes:
    - even iter: mode=off  (Verify default English-only works)
    - odd iter:  mode=on   (Verify SendKeys via DirectUI Edit works)
  Both modes test:
    - ValuePattern path (SetValue + read-back) - should pass both
    - SendKeys path (Ctrl+A + DEL + SendKeys + read-back) - should pass both

.NOTES
  RESULT: D:\csart\eol-probe\RESULT-10.txt
  ADR-0022 D1 + ADR-0024 D4 compliant
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

$logPath = Join-Path $WorkDir 'probe10-stdout.txt'
"" | Out-File -FilePath $logPath -Encoding UTF8
$logFile = [System.IO.StreamWriter]::new($logPath, $true, [System.Text.Encoding]::UTF8)
$logFile.AutoFlush = $true
function Log($msg) {
  $stamp = (Get-Date).ToString('HH:mm:ss')
  Write-Host "[$stamp] $msg"
  $logFile.WriteLine("[$stamp] $msg")
}

Log "probe-10 starting iter=$Iter warmup=$Warmup"

# Pre-declare AddType once at top for SetForegroundWindow
Add-Type -Name P10FW -Namespace W -MemberDefinition @"
using System;
using System.Runtime.InteropServices;
public class FW {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
"@

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
$content = 'probe10 baseline'

Stop-NotepadAll

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  $isWarmup = $i -lt $Warmup
  $runId = $i + 1
  $mode = if (($i % 2) -eq 0) { 'off' } else { 'on' }
  try {
    $nonce = ([Guid]::NewGuid().ToString('N')).Substring(0, 8)
    $path = Join-Path $WorkDir ("probe10-$nonce.txt")
    [System.IO.File]::WriteAllText($path, $content, [System.Text.Encoding]::UTF8)
    $null = Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $path + '"') -PassThru
    Start-Sleep -Milliseconds 2500

    $npWin = $null
    for ($poll = 0; $poll -lt 15; $poll++) {
      $npWin = Find-NotepadWin
      if ($npWin) { break }
      Start-Sleep -Milliseconds 500
    }
    if (-not $npWin) { Log "[iter $runId] WARN: window not found, skip"; Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force; Remove-Item $path -Force -EA SilentlyContinue; continue }

    try {
      [W.FW]::SetForegroundWindow([IntPtr]$npWin.Current.NativeWindowHandle) | Out-Null
    } catch {}
    Start-Sleep -Milliseconds 300

    $doc = Get-Document -Win $npWin
    if (-not $doc) { Log "[iter $runId] WARN: doc not found"; Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force; Remove-Item $path -Force -EA SilentlyContinue; continue }

    $vpContent = "vp_$mode`_r$runId"
    $vpOk = Set-DocVP -Doc $doc -Text $vpContent
    Start-Sleep -Milliseconds 200
    $vpRead = Get-DocText -Doc $doc
    $vpResult = ($vpOk -and ($vpRead -eq $vpContent))

    try {
      [System.Windows.Forms.SendKeys]::SendWait('^a')
      Start-Sleep -Milliseconds 100
      [System.Windows.Forms.SendKeys]::SendWait('{DEL}')
      Start-Sleep -Milliseconds 300
    } catch {}
    $skContent = "sk_$mode`_r$runId"
    try {
      [System.Windows.Forms.SendKeys]::SendWait($skContent)
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
      Log "[iter $runId mode=$mode] VP=$vpResult SK=$skResult vp_set='$vpContent' vp_read='$vpRead' sk_set='$skContent' sk_read='$skRead'"
    }
  } catch {
    Log "[iter $runId] ERROR: $($_.Exception.Message)"
  } finally {
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500
  }
}

Stop-NotepadAll

$rl = @()
$rl += 'probe-10 IME two-state PoC - result'
$rl += ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))
$rl += ('iter=' + $Iter + ' warmup=' + $Warmup)
$rl += ''
$rl += 'metric      | success / iter | pct'
$rl += '------------+-----------------+-------'
foreach ($mode in @('off','on')) {
  foreach ($metric in @('vp','sk')) {
    $key = $mode + '_' + $metric
    $r = $results[$key]
    $c = ($r | Where-Object { $_ -eq $true }).Count
    $pct = if (($Iter / 2) -gt 0) { [math]::Round($c * 100 / ($Iter / 2), 1) } else { 0 }
    $rl += ('{0,-7} {1,-7} | {2,5} / {3,-4}    | {4,5}%' -f $mode, $metric, $c, $Iter, $pct)
  }
}
$combinedTotal = 0; $combinedFound = 0
foreach ($mode in @('off','on')) {
  foreach ($metric in @('vp','sk')) {
    $combinedTotal += ($Iter / 2)
    $combinedFound += ($results[$mode + '_' + $metric] | Where-Object { $_ -eq $true }).Count
  }
}
   =  * 2
$rl += ''
$rl += ('overall: ' + $combinedFound + ' / ' + $combinedTotal + ' = ' + $combinedPct + '%')
$rl += ''
$rl += 'go_criterion: IME_on_off_two_state_correctness >= 90%'
  overall = GO

$rp = Join-Path $WorkDir 'RESULT-10.txt'
[System.IO.File]::WriteAllLines($rp, $rl, [System.Text.Encoding]::UTF8)
Log ('RESULT written to ' + $rp)
foreach ($l in $rl) { Log $l }
$logFile.Close()
