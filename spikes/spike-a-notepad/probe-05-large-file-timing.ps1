<#
.SYNOPSIS
  probe-05: Large-file read/write timing + memory delta.

.DESCRIPTION
  Spike A B1.2: close stage-0 DoD carry-over #3 -- go criterion "1 MB read <= 2 s
  AND memory delta <= 100 MB".

  Three sizes x 10 iterations (with 2 warmup) each:
    - read time     : ValuePattern.GetValue latency (milliseconds)
    - write time    : ValuePattern.SetValue latency (milliseconds)
    - mem delta     : Notepad WorkingSet64 before/after operation (MB)

  Per ADR-0022 D1 locate window via ClassName=Notepad + TabItem.Name LIKE nonce.
  Per ADR-0024 D4 PURE ASCII.
  RESULT: D:\csart\eol-probe\RESULT-05.txt
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

# open file with output redirection so we can monitor progress
$logPath = Join-Path $WorkDir 'probe05-stdout.txt'
"" | Out-File -FilePath $logPath -Encoding UTF8
$logFile = [System.IO.StreamWriter]::new($logPath, $true, [System.Text.Encoding]::UTF8)
$logFile.AutoFlush = $true
function Log($msg) {
  $stamp = (Get-Date).ToString('HH:mm:ss')
  $line = "[$stamp] $msg"
  Write-Host $line
  $logFile.WriteLine($line)
}

Log "probe-05 starting (iter=$Iter warmup=$Warmup)"

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
  param([string]$Dir, [int]$SizeBytes)
  if (-not (Test-Path $Dir)) { New-Item -ItemType Directory -Path $Dir -Force | Out-Null }
  $nonce = ([Guid]::NewGuid().ToString('N')).Substring(0, 8)
  $path  = Join-Path $Dir ("probe05-{0}-{1}.txt" -f $nonce, $SizeBytes)
  [System.IO.File]::WriteAllText($path, ('a' * $SizeBytes))
  return $path
}

function Get-StableMemoryMB {
  $last = 0
  for ($i = 0; $i -lt 3; $i++) {
    $p = Get-Process Notepad -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($p) { $last = [math]::Round($p.WorkingSet64 / 1MB, 2) }
    Start-Sleep -Milliseconds 100
  }
  return $last
}

function Find-NotepadWindowByNonce {
  param([string]$Nonce)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  $CT = [System.Windows.Automation.ControlType]
  $root = $AE::RootElement
  $kids = $root.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)
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

function Get-ValueMs {
  param([System.Windows.Automation.AutomationElement]$Doc)
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $vp = $Doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
  $null = $vp.CurrentValue
  $sw.Stop()
  return [math]::Round($sw.Elapsed.TotalMilliseconds, 2)
}

function Set-ValueMs {
  param(
    [System.Windows.Automation.AutomationElement]$Doc,
    [string]$NewContent
  )
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $vp = $Doc.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
  $vp.SetValue($NewContent)
  $sw.Stop()
  return [math]::Round($sw.Elapsed.TotalMilliseconds, 2)
}

function Get-MedianMinMax {
  param([double[]]$Values)
  if ($Values.Count -eq 0) {
    return [pscustomobject]@{ Min = 0; Median = 0; Max = 0 }
  }
  $sorted = $Values | Sort-Object
  $n = $sorted.Count
  $min = $sorted[0]
  $max = $sorted[$n - 1]
  $median = if ($n % 2 -eq 1) { $sorted[[math]::Floor($n / 2)] }
            else { ($sorted[$n/2 - 1] + $sorted[$n/2]) / 2 }
  return [pscustomobject]@{ Min = [math]::Round($min, 2); Median = [math]::Round($median, 2); Max = [math]::Round($max, 2) }
}

# ---- main ----

Stop-NotepadAll
$sizes = @(
  @{ Name = '1KB'; Bytes = 1024 }
  @{ Name = '100KB'; Bytes = 102400 }
  @{ Name = '1MB'; Bytes = 1048576 }
)
$results = @()

foreach ($s in $sizes) {
  $sizeName = $s.Name
  $sizeBytes = $s.Bytes
  $readTimes = @()
  $writeTimes = @()
  $readMemDeltas = @()
  $writeMemDeltas = @()

  for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
    $isWarmup = $i -lt $Warmup
    $path = New-NonceFile -Dir $WorkDir -SizeBytes $sizeBytes
    $nonceTag = Split-Path -Leaf $path

    try {
      Log ("[$sizeName iter $($i+1)/$($Iter+$Warmup)] launching notepad $path")
      $null = Start-Process -FilePath 'notepad.exe' -ArgumentList "`"$path`"" -PassThru

      # Poll up to 15s for the window
      $win = $null
      $doc = $null
      for ($poll = 0; $poll -lt 30; $poll++) {
        Start-Sleep -Milliseconds 500
        $win = Find-NotepadWindowByNonce -Nonce $nonceTag
        if ($win) {
          $doc = Find-DocumentInWindow -Win $win
          if ($doc) { break }
        }
      }

      if (-not $doc) {
        Log ("[$sizeName iter $($i+1)] WARN: doc not found after 15s, skip")
        continue
      }

      $memBefore = Get-StableMemoryMB
      $readMs = Get-ValueMs -Doc $doc
      $memAfterRead = Get-StableMemoryMB
      $readDelta = [math]::Round($memAfterRead - $memBefore, 2)

      $newContent = ('b' * $sizeBytes)
      $writeMs = Set-ValueMs -Doc $doc -NewContent $newContent
      $memAfterWrite = Get-StableMemoryMB
      $writeDelta = [math]::Round($memAfterWrite - $memBefore, 2)

      if (-not $isWarmup) {
        $readTimes += $readMs
        $writeTimes += $writeMs
        $readMemDeltas += $readDelta
        $writeMemDeltas += $writeDelta
      }

      Log ("[$sizeName iter $($i+1)] read=${readMs}ms write=${writeMs}ms read_dMB=${readDelta} write_dMB=${writeDelta} mem_before=${memBefore}MB")

      Close-Window -Win $win
      Start-Sleep -Milliseconds 300
    } catch {
      Log ("[$sizeName iter $($i+1)] ERROR: $($_.Exception.Message)")
    } finally {
      Remove-Item -Path $path -Force -ErrorAction SilentlyContinue
    }
  }

  $readStat = Get-MedianMinMax -Values $readTimes
  $writeStat = Get-MedianMinMax -Values $writeTimes
  $readMemStat = Get-MedianMinMax -Values $readMemDeltas
  $writeMemStat = Get-MedianMinMax -Values $writeMemDeltas

  $results += [pscustomobject]@{
    Size = $sizeName
    Bytes = $sizeBytes
    ReadMs = $readStat
    WriteMs = $writeStat
    ReadMemDeltaMB = $readMemStat
    WriteMemDeltaMB = $writeMemStat
  }
  Log ("[$sizeName] done: read_med=${readStat.Median}ms write_med=${writeStat.Median}ms write_dMB_med=${writeMemStat.Median}MB")
}

Stop-NotepadAll

# ---- report ----
$reportLines = @(
  'probe-05 large-file timing - result',
  ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')),
  ('iter=' + $Iter + ' warmup=' + $Warmup),
  '',
  'size    | read_ms (min/med/max) | write_ms (min/med/max) | read_dMB (min/med/max) | write_dMB (min/med/max)',
  '--------+----------------------+------------------------+------------------------+-------------------------'
)
foreach ($r in $results) {
  $reportLines += ("{0,-7}| {1,5}/{2,5}/{3,5}         | {4,5}/{5,5}/{6,5}              | {7,5}/{8,5}/{9,5}              | {10,5}/{11,5}/{12,5}" -f `
    $r.Size,
    $r.ReadMs.Min, $r.ReadMs.Median, $r.ReadMs.Max,
    $r.WriteMs.Min, $r.WriteMs.Median, $r.WriteMs.Max,
    $r.ReadMemDeltaMB.Min, $r.ReadMemDeltaMB.Median, $r.ReadMemDeltaMB.Max,
    $r.WriteMemDeltaMB.Min, $r.WriteMemDeltaMB.Median, $r.WriteMemDeltaMB.Max)
}
$reportLines += ''
$reportLines += 'go_criterion (1MB): read_ms_median <= 2000 AND write_dMB_median <= 100'
$mb1 = $results | Where-Object { $_.Size -eq '1MB' }
if ($mb1) {
  $pass = ($mb1.ReadMs.Median -le 2000) -and ($mb1.WriteMemDeltaMB.Median -le 100)
  $reportLines += ('  1MB_read_median   = {0} ms  (limit 2000)  -> {1}' -f $mb1.ReadMs.Median, $(if ($mb1.ReadMs.Median -le 2000) {'PASS'} else {'FAIL'}))
  $reportLines += ('  1MB_write_dMB_med = {0} MB  (limit 100)    -> {1}' -f $mb1.WriteMemDeltaMB.Median, $(if ($mb1.WriteMemDeltaMB.Median -le 100) {'PASS'} else {'FAIL'}))
  $reportLines += ('  overall = {0}' -f $(if ($pass) {'GO'} else {'NO-GO'}))
}

$reportPath = Join-Path $WorkDir 'RESULT-05.txt'
[System.IO.File]::WriteAllLines($reportPath, $reportLines, [System.Text.Encoding]::UTF8)
Log "RESULT written to $reportPath"
foreach ($l in $reportLines) { Log $l }
$logFile.Close()
