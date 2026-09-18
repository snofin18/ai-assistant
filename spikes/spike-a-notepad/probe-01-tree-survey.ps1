# ============================================================================
# SPIKE-A / TASK-002 -- Notepad UIA recon script (one-shot verification code, **not product code**)
#
# Belongs to: TASK-002 (Spike A: Notepad UIA probe + API archeology), write scope = spikes/spike-a-notepad/**
# Why PowerShell and not Rust: Windows ships UIAutomationClient / UIAutomationTypes
#   assemblies out of the box, **zero 3rd-party deps** for control-tree survey + timing; whereas card step 3
#   asks for the Rust `windows` crate + `uiautomation` = "add 3rd-party deps", which per AGENTS.md Sec.4
#   drift trigger (1) requires registration + human approval (see docs/DEPENDENCIES.md and PL-019).
#   -> Use this script first to collect data, then write a Rust spike to verify the **production path**
#   (go/no-go is decided by Rust).
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File <this-file>
# Side effects: starts and **force-closes** Notepad; only writes a temp .txt in %TEMP% and self-cleans.
# Note: the script runs `Stop-Process Notepad` first, **save any manually-opened Notepad content before**.
#
# !! Chinese characters in the console may render as mojibake (PowerShell console codepage issue);
#    **judge by script-internal comparisons (True/False) and lengths, not by reading the console**.
# ============================================================================

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$CT = [System.Windows.Automation.ControlType]
$walk = [System.Windows.Automation.TreeWalker]::ControlViewWalker

$tmp = Join-Path $env:TEMP "spike-a-notepad-probe.txt"
$body = "line1 ASCII probe`r`nline2 中文探针 你好世界`r`nline3 mixed 混合 12345`r`n"
[IO.File]::WriteAllText($tmp, $body, (New-Object Text.UTF8Encoding($false)))
$launched = Start-Process notepad -ArgumentList "`"$tmp`"" -PassThru
Write-Output ("launched stub: name={0} pid={1}" -f $launched.ProcessName, $launched.Id)
Start-Sleep -Seconds 2
try { $lp = Get-Process -Id $launched.Id -ErrorAction Stop; Write-Output ("  stub path={0} exited={1}" -f $lp.Path, $lp.HasExited) } catch { Write-Output "  stub already exited" }

$found = $null
for ($i = 0; $i -lt 20 -and -not $found; $i++) {
  $kids = $AE::RootElement.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)
  foreach ($k in $kids) { if ($k.Current.Name -like "*spike-a-notepad-probe*") { $found = $k; break } }
  if (-not $found) { Start-Sleep -Milliseconds 700 }
}
if (-not $found) { Write-Output "WINDOW NOT FOUND after 14s"; exit 1 }
$c = $found.Current
$owner = Get-Process -Id $c.ProcessId
Write-Output ("window: name='{0}'" -f $c.Name)
Write-Output ("        class={0} ct={1} aid='{2}'" -f $c.ClassName, $c.ControlType.ProgrammaticName, $c.AutomationId)
Write-Output ("        rect={0} nativeWinHandle={1}" -f $c.BoundingRectangle, $c.NativeWindowHandle)
Write-Output ("        OWNER pid={0} name={1}" -f $c.ProcessId, $owner.ProcessName)
Write-Output ("        OWNER path={0}" -f $owner.Path)
Write-Output ("        stubPid={0} == ownerPid ? {1}" -f $launched.Id, ($launched.Id -eq $c.ProcessId))

$script:nodes = 0
$script:lines = New-Object System.Collections.Generic.List[string]
function Walk($el, $depth) {
  if ($depth -gt 8 -or $script:nodes -ge 150) { return }
  $script:nodes++
  $cur = $el.Current
  $nm = ($cur.Name -replace "[\r\n]+", " ")
  if ($nm.Length -gt 34) { $nm = $nm.Substring(0, 34) }
  $script:lines.Add(("{0}{1,-14} aid='{2}' cls={3} name='{4}'" -f ("  " * $depth), $cur.ControlType.ProgrammaticName.Split('.')[-1], $cur.AutomationId, $cur.ClassName, $nm))
  $ch = $walk.GetFirstChild($el)
  while ($ch) { Walk $ch ($depth + 1); $ch = $walk.GetNextSibling($ch) }
}
$sw = [Diagnostics.Stopwatch]::StartNew(); Walk $found 0; $sw.Stop()
Write-Output ("fullTreeWalk: {0:N1} ms  nodes={1}" -f $sw.Elapsed.TotalMilliseconds, $script:nodes)
$script:lines | Select-Object -First 50 | ForEach-Object { Write-Output $_ }

$cDoc = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Document)
$cEdt = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Edit)
$editCond = New-Object System.Windows.Automation.OrCondition -ArgumentList @($cDoc, $cEdt)
$sw = [Diagnostics.Stopwatch]::StartNew()
$edit = $found.FindFirst($TS::Descendants, $editCond)
$sw.Stop()
Write-Output ("findEdit: {0:N1} ms found={1}" -f $sw.Elapsed.TotalMilliseconds, ($edit -ne $null))
if ($edit) {
  $ec = $edit.Current
  Write-Output ("  edit: ct={0} aid='{1}' cls={2}" -f $ec.ControlType.ProgrammaticName.Split('.')[-1], $ec.AutomationId, $ec.ClassName)
  $vp = $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
  $sw = [Diagnostics.Stopwatch]::StartNew(); $val = $vp.Current.Value; $sw.Stop()
  Write-Output ("  ValuePattern.GetValue: {0:N2} ms len={1} isReadOnly={2}" -f $sw.Elapsed.TotalMilliseconds, $val.Length, $vp.Current.IsReadOnly)
  $norm = { param($s) ($s -replace "`r`n", "`n").TrimEnd("`n") }
  Write-Output ("  value == file content ? {0}" -f ((& $norm $val) -eq (& $norm $body)))
  Write-Output ("  value line2 = '{0}'" -f ((& $norm $val) -split "`n")[1])
  $tp = $null
  if ($edit.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$tp)) { Write-Output "  TextPattern: SUPPORTED" } else { Write-Output "  TextPattern: NOT supported" }
}
Stop-Process -Id $c.ProcessId -Force
Write-Output "notepad closed"
