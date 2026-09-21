<#
.SYNOPSIS
  probe-09: Key control locate rate PoC (B1.6).
.DESCRIPTION
  Spike A B1.6: close stage-0 DoD carry-over = "key control location rate >= 90%".
  6 key controls, 12 iter (10 + 2 warmup), find via ordered candidate chain.

.NOTES
  CONSTRAINT (ADR-0022 D1): Start-Process PID is never used.
  CONSTRAINT (ADR-0024 D4): PURE ASCII. CJK via [char] code points.
  RESULT: D:\csart\eol-probe\RESULT-09.txt
#>

param(
  [string]$WorkDir = 'D:\csart\eol-probe',
  [int]$Iter = 10,
  [int]$Warmup = 2
)

$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$logPath = Join-Path $WorkDir 'probe09-stdout.txt'
"" | Out-File -FilePath $logPath -Encoding UTF8
$logFile = [System.IO.StreamWriter]::new($logPath, $true, [System.Text.Encoding]::UTF8)
$logFile.AutoFlush = $true
function Log($msg) {
  $stamp = (Get-Date).ToString('HH:mm:ss')
  $line = "[$stamp] $msg"
  Write-Host $line
  $logFile.WriteLine($line)
}

Log 'probe-09 starting (iter=' + $Iter + ' warmup=' + $Warmup + ')'

$CN_FILE = [char]0x6587 + [char]0x4EF6

# Key controls spec: each key maps to an array of candidate descriptors.
# Each descriptor: control_type, class, aid, name.
# We try each candidate in order, return first match.
$SPEC = @{
  file_menu   = @(
    @{ ct='MenuItem'; name=$CN_FILE },
    @{ ct='MenuItem'; aid='MenuBar' }
  )
  edit_area   = @(
    @{ ct='Document'; class='RichEditD2DPT' },
    @{ ct='Edit';     class='RichEditD2DPT' },
    @{ ct='Document' }
  )
  tabview     = @(
    @{ ct='Tab';  aid='Tabs' },
    @{ ct='Tab';  aid='TabListView' },
    @{ ct='List'; aid='TabListView' }
  )
  statusbar   = @(
    @{ ct='Text'; aid='ContentTextBlock' },
    @{ ct='Static'; aid='ContentTextBlock' }
  )
  closebutton = @(
    @{ ct='Button'; aid='CloseButton' },
    @{ ct='Button'; name=$CN_FILE }
  )
  addbutton   = @(
    @{ ct='Button'; aid='AddButton' }
  )
}
$KEYS = @('file_menu','edit_area','tabview','statusbar','closebutton','addbutton')

function Stop-NotepadAll {
  Get-Process Notepad -ErrorAction SilentlyContinue | ForEach-Object {
    try { $_.CloseMainWindow() | Out-Null } catch {}
    Start-Sleep -Milliseconds 200
    if (-not $_.HasExited) { Stop-Process -Id $_.Id -Force }
  }
  Start-Sleep -Milliseconds 500
}
function New-NonceFile {
  param([string]$Dir,[string]$Tag,[string]$Content)
  if (-not (Test-Path $Dir)) { New-Item -ItemType Directory -Path $Dir -Force | Out-Null }
  $nonce = ([Guid]::NewGuid().ToString('N')).Substring(0,8)
  $path = Join-Path $Dir ('probe09-' + $Tag + '-' + $nonce + '.txt')
  [System.IO.File]::WriteAllText($path, $Content)
  return @{ Path=$path; Nonce='probe09-' + $Tag + '-' + $nonce }
}
function Find-NotepadWindowByNonce {
  param([string]$Nonce)
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  foreach ($k in $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
    if ($k.Current.Name -like ('*' + $Nonce + '*')) { return $k }
  }
  return $null
}
function Resolve-FirstMatch {
  param($NotepadWin, [Parameter()]$Candidates)
  $AE = [System.Windows.Automation.AutomationElement]
  $CT = [System.Windows.Automation.ControlType]
  $TS = [System.Windows.Automation.TreeScope]
  if ($null -eq $Candidates) { return $null }
  foreach ($d in $Candidates) {
    if ($null -eq $d) { continue }
    $ct = $null
    switch ($d.ct) {
      'Document'  { $ct = $CT::Document }
      'Edit'      { $ct = $CT::Edit }
      'Button'    { $ct = $CT::Button }
      'MenuItem'  { $ct = $CT::MenuItem }
      'Tab'       { $ct = $CT::Tab }
      'List'      { $ct = $CT::List }
      'ListItem'  { $ct = $CT::ListItem }
      'Text'      { $ct = $CT::Text }
      'StatusBar' { $ct = $CT::StatusBar }
      'Window'    { $ct = $CT::Window }
      'Static'    { $ct = $CT::Text }  # Static maps to Text in modern UIA
      default     { $ct = $null }
    }
    $conds = @()
    if ($null -ne $ct) {
      $conds += New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $ct)
    }
    if ($d.class) { $conds += New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, $d.class) }
    if ($d.aid)   { $conds += New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, $d.aid) }
    if ($d.name)  { $conds += New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $d.name) }
    if ($conds.Count -eq 0) { continue }
    $andCond = $conds[0]
    for ($i=1; $i -lt $conds.Count; $i++) {
      $andCond = New-Object System.Windows.Automation.AndCondition($andCond, $conds[$i])
    }
    try {
      $hit = $NotepadWin.FindFirst($TS::Descendants, $andCond)
      if ($hit) { return $hit }
    } catch {}
  }
  return $null
}

Stop-NotepadAll

$results = @{}
foreach ($k in $KEYS) { $results[$k] = @{ found=@(); latency=@() } }

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  $isWarmup = $i -lt $Warmup
  $runId = $i + 1
  Stop-NotepadAll
  $src = New-NonceFile -Dir $WorkDir -Tag 'src' -Content 'B1.6 probe-09 key control locate PoC'
  $srcPath = $src.Path
  $nonce = $src.Nonce
  $notepadWin = $null
  try {
    $null = Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $srcPath + '"') -PassThru
    for ($poll = 0; $poll -lt 30; $poll++) {
      Start-Sleep -Milliseconds 500
      $notepadWin = Find-NotepadWindowByNonce -Nonce $nonce
      if ($notepadWin) { break }
    }
    if (-not $notepadWin) { Log ('[iter ' + $runId + '] WARN: window not found, skip'); continue }
    Start-Sleep -Milliseconds 500
    foreach ($k in $KEYS) {
      $sw = [System.Diagnostics.Stopwatch]::StartNew()
      $hit = Resolve-FirstMatch -NotepadWin $notepadWin -Candidates $SPEC[$k]
      $sw.Stop()
      $lat = [math]::Round($sw.Elapsed.TotalMilliseconds, 2)
      if (-not $isWarmup) {
        $results[$k].found += ($null -ne $hit)
        $results[$k].latency += if ($null -ne $hit) { $lat } else { 0 }
      }
    }
    if (-not $isWarmup) {
      $parts = @()
      foreach ($k in $KEYS) { $parts += ($k + '=' + $results[$k].found[-1]) }
      Log ('[iter ' + $runId + '] found: ' + ($parts -join ' '))
    }
  } catch {
    Log ('[iter ' + $runId + '] ERROR: ' + $_.Exception.Message)
  } finally {
    Get-Process Notepad -ErrorAction SilentlyContinue | ForEach-Object {
      try { $_.CloseMainWindow() | Out-Null } catch {}
      Start-Sleep -Milliseconds 200
      if (-not $_.HasExited) { Stop-Process -Id $_.Id -Force }
    }
    Remove-Item -Path $srcPath -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
  }
}
Stop-NotepadAll

function Stat {
  param([double[]]$A)
  if ($A.Count -eq 0) { return [pscustomobject]@{Count=0;Median=0;Miss=0} }
  $s = $A | Sort-Object
  $n = $s.Count
  $med = if ($n % 2 -eq 1) { $s[[math]::Floor($n/2)] } else { ($s[$n/2-1] + $s[$n/2]) / 2 }
  return [pscustomobject]@{Count=$n;Median=[math]::Round($med,2);Miss=($A | Where-Object { $_ -eq 0 }).Count}
}

$rl = @()
$rl += 'probe-09 key control locate rate - result'
$rl += ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))
$rl += ('iter=' + $Iter + ' warmup=' + $Warmup)
$rl += ''
$rl += 'control        | found / iter | pct  | median ms'
$rl += '---------------+--------------+------+----------'
$overallFound = 0
$overallTotal = $Iter * $KEYS.Count
foreach ($k in $KEYS) {
  $r = $results[$k]
  $fc = ($r.found | Where-Object { $_ -eq $true }).Count
  $pct = if ($Iter -gt 0) { [math]::Round($fc * 100 / $Iter, 1) } else { 0 }
  $s = Stat $r.latency
  $rl += ('{0,-14} | {1,5} / {2,-4} | {3,5}% | {4}' -f $k, $fc, $Iter, $pct, $s.Median)
  $rl += ('  (miss=' + $s.Miss + ')')
  $overallFound += $fc
}
$overallPct = if ($overallTotal -gt 0) { [math]::Round($overallFound * 100 / $overallTotal, 1) } else { 0 }
$rl += ''
$rl += ('overall: ' + $overallFound + ' / ' + $overallTotal + ' = ' + $overallPct + '%')
$rl += ''
$rl += 'go_criterion: key_control_location_rate >= 90%'
$rl += ('  overall = ' + $(if ($overallPct -ge 90) {'GO'} else {'NO-GO (' + $overallPct + '% < 90%)'}))

$rp = Join-Path $WorkDir 'RESULT-09.txt'
[System.IO.File]::WriteAllLines($rp, $rl, [System.Text.Encoding]::UTF8)
Log ('RESULT written to ' + $rp)
foreach ($l in $rl) { Log $l }
$logFile.Close()