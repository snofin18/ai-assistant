<#
.SYNOPSIS
  probe-09 v3: Key control locate rate PoC (B1.6) - clean rewrite (no StreamWriter).
.DESCRIPTION
  6 key controls on Win11 25H2 modern Notepad main window.
  10 iter + 2 warmup, find via ordered candidate chain.
.NOTES
  Pure ASCII. ADR-0024 D4 compliant.
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

function Stop-NotepadAll {
  Get-Process Notepad -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Milliseconds 500
}
function Find-NotepadWin {
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  foreach ($k in $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
    $proc = Get-Process -Id $k.Current.ProcessId -ErrorAction SilentlyContinue
    if ($proc -and $proc.ProcessName -eq 'Notepad' -and $k.Current.Name -match 'probe09-') { return $k }
  }
  return $null
}

# Key controls spec
$CN_FILE = [char]0x6587 + [char]0x4EF6
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
    @{ ct='Button'; aid='CloseButton' }
  )
  addbutton   = @(
    @{ ct='Button'; aid='AddButton' }
  )
}
$KEYS = @('file_menu','edit_area','tabview','statusbar','closebutton','addbutton')

function Resolve-FirstMatch {
  param($Win, $Candidates)
  if (-not $Win) { return $null }
  $AE = [System.Windows.Automation.AutomationElement]
  $TS = [System.Windows.Automation.TreeScope]
  function ResolveRecurse {
    param($Parent, $Descs)
    foreach ($d in $Descs) {
      $ct = $null
      switch ($d.ct) {
        'Document'  { try { $ct = [System.Windows.Automation.ControlType]::Document } catch {} }
        'Edit'      { try { $ct = [System.Windows.Automation.ControlType]::Edit } catch {} }
        'Button'    { try { $ct = [System.Windows.Automation.ControlType]::Button } catch {} }
        'MenuItem'  { try { $ct = [System.Windows.Automation.ControlType]::MenuItem } catch {} }
        'Tab'       { try { $ct = [System.Windows.Automation.ControlType]::Tab } catch {} }
        'List'      { try { $ct = [System.Windows.Automation.ControlType]::List } catch {} }
        'Text'      { try { $ct = [System.Windows.Automation.ControlType]::Text } catch {} }
        'Static'    { try { $ct = [System.Windows.Automation.ControlType]::Text } catch {} }
      }
      $conds = @()
      if ($null -ne $ct) { $conds += New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $ct) }
      if ($d.class) { $conds += New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, $d.class) }
      if ($d.aid)   { $conds += New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, $d.aid) }
      if ($d.name)  { $conds += New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $d.name) }
      if ($conds.Count -eq 0) { continue }
      $andCond = $conds[0]
      for ($i=1; $i -lt $conds.Count; $i++) {
        $andCond = New-Object System.Windows.Automation.AndCondition($andCond, $conds[$i])
      }
      try {
        $hit = $Parent.FindFirst($TS::Descendants, $andCond)
        if ($hit) { return $hit }
      } catch {}
    }
    return $null
  }
  return ResolveRecurse $Win $Candidates
}

Remove-Item -Path (Join-Path $WorkDir 'probe09-stdout.txt') -ErrorAction SilentlyContinue
Remove-Item -Path (Join-Path $WorkDir 'RESULT-09.txt') -ErrorAction SilentlyContinue

$results = @{}
foreach ($k in $KEYS) { $results[$k] = @{ found=@(); latency=@() } }
Stop-NotepadAll

for ($i = 0; $i -lt ($Iter + $Warmup); $i++) {
  $isWarmup = $i -lt $Warmup
  $runId = $i + 1
  Stop-NotepadAll
  $nonce = ([Guid]::NewGuid().ToString('N')).Substring(0, 8)
  $path = Join-Path $WorkDir ('probe09-' + $nonce + '.txt')
  [System.IO.File]::WriteAllText($path, 'B1.6 probe-09 test content', [System.Text.Encoding]::UTF8)
  $null = Start-Process -FilePath 'notepad.exe' -ArgumentList ('"' + $path + '"') -PassThru
  Start-Sleep -Milliseconds 2500
  $npWin = $null
  for ($poll = 0; $poll -lt 20; $poll++) {
    $npWin = Find-NotepadWin
    if ($npWin) { break }
    Start-Sleep -Milliseconds 500
  }
  if (-not $npWin) { continue }
  foreach ($k in $KEYS) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $hit = Resolve-FirstMatch -Win $npWin -Candidates $SPEC[$k]
    $sw.Stop()
    $found = ($null -ne $hit)
    $lat = [math]::Round($sw.Elapsed.TotalMilliseconds, 2)
    if (-not $isWarmup) {
      $results[$k].found += $found
      $results[$k].latency += if ($found) { $lat } else { 0 }
    }
    Write-Host ('[' + $k + '] iter ' + $runId + ': ' + $found + ' (' + $lat + 'ms)')
  }
  Remove-Item -Path $path -Force -ErrorAction SilentlyContinue
}
Stop-NotepadAll

$rl = @()
$rl += 'probe-09 v3 key control locate rate - result'
$rl += ('generated_utc=' + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))
$rl += ('iter=' + $Iter + ' warmup=' + $Warmup)
$rl += ''
$rl += 'control        | found / iter | pct  | median ms'
$rl += '---------------+--------------+------+----------'
$totalFound = 0
$totalTests = $Iter * $KEYS.Count
foreach ($k in $KEYS) {
  $r = $results[$k]
  $c = ($r.found | Where-Object { $_ -eq $true }).Count
  $pct = if ($Iter -gt 0) { [math]::Round($c * 100 / $Iter, 1) } else { 0 }
  $median = if ($r.latency.Count -gt 0) { $med = ($r.latency | Sort-Object)[$r.latency.Count / 2]; [math]::Round($med, 2) } else { 0 }
  $rl += ('{0,-14} | {1,5} / {2,-4}   | {3,5}% | {4}' -f $k, $c, $Iter, $pct, $median)
  $totalFound += $c
}
$overallPct = if ($totalTests -gt 0) { [math]::Round($totalFound * 100 / $totalTests, 1) } else { 0 }
$rl += ''
$rl += ('overall: ' + $totalFound + ' / ' + $totalTests + ' = ' + $overallPct + '%')
$rl += ''
$rl += 'go_criterion: key_control_location_rate >= 90%'
if ($overallPct -ge 90) { $rl += '  overall = GO' } else { $rl += '  overall = NO-GO' }

$rp = Join-Path $WorkDir 'RESULT-09.txt'
[System.IO.File]::WriteAllLines($rp, $rl, [System.Text.Encoding]::UTF8)
Write-Host ''
Write-Host ('RESULT written to ' + $rp)
foreach ($l in $rl) { Write-Host $l }