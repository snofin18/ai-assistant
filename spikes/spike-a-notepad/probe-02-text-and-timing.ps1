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

Get-Process Notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
Write-Output ("notepad procs after kill = {0}" -f (@(Get-Process Notepad -ErrorAction SilentlyContinue)).Count)

$stamp = Get-Date -Format "HHmmss"
$tmp = Join-Path $env:TEMP "spike-a-$stamp.txt"
$body = "L1 ASCII marker $stamp`r`nL2 中文探针 你好世界`r`nL3 mixed 混合 12345`r`n"
[IO.File]::WriteAllText($tmp, $body, (New-Object Text.UTF8Encoding($false)))
Write-Output ("file: {0}  chars={1}  bytes={2}" -f $tmp, $body.Length, (Get-Item $tmp).Length)

$p = Start-Process notepad -ArgumentList "`"$tmp`"" -PassThru
Start-Sleep -Seconds 5
$cDoc = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Document)
$cEdt = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Edit)
$editCond = New-Object System.Windows.Automation.OrCondition -ArgumentList @($cDoc, $cEdt)
$nameCond = New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, "spike-a-$stamp.txt - Notepad")
$win = $AE::RootElement.FindFirst($TS::Children, $nameCond)
if (-not $win) { Write-Output "window not found"; exit 1 }
Write-Output ("window owner pid={0}  launched stub pid={1}  same={2}" -f $win.Current.ProcessId, $p.Id, ($win.Current.ProcessId -eq $p.Id))
$edit = $win.FindFirst($TS::Descendants, $editCond)
$vp = $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
$tp = $null; [void]$edit.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$tp)

$val = $vp.Current.Value
$vis = $val.Replace("`r","<CR>").Replace("`n","<LF>")
Write-Output ("ValuePattern len={0}" -f $val.Length)
Write-Output ("ValuePattern raw={0}" -f $vis)
$normAll = { param($s) $s.Replace("`r`n","`n").Replace("`r","`n").TrimEnd("`n") }
Write-Output ("  == file content (normalize CR/LF/CRLF) ? {0}" -f ((& $normAll $val) -eq (& $normAll $body)))
if ($tp) {
  $t = $tp.DocumentRange.GetText(-1)
  Write-Output ("TextPattern  len={0}" -f $t.Length)
  Write-Output ("TextPattern  raw={0}" -f $t.Replace("`r","<CR>").Replace("`n","<LF>"))
  Write-Output ("  == ValuePattern ? {0}" -f ($t -eq $val))
}

# ---- median: 10 full-tree walks + 10 GetValue calls ----
function Median($a) { $s = $a | Sort-Object; $n = $s.Count; if ($n % 2) { $s[[int](($n-1)/2)] } else { ($s[$n/2-1] + $s[$n/2]) / 2 } }
$tw = @(); $gv = @(); $fe = @()
for ($i = 0; $i -lt 10; $i++) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $n = 0; $st = New-Object System.Collections.Stack; $st.Push(@($win, 0))
  while ($st.Count) { $it = $st.Pop(); $n++; $ch = $walk.GetFirstChild($it[0]); while ($ch) { $st.Push(@($ch, 0)); $ch = $walk.GetNextSibling($ch) } }
  $sw.Stop(); $tw += $sw.Elapsed.TotalMilliseconds
  $sw = [Diagnostics.Stopwatch]::StartNew(); $x = $win.FindFirst($TS::Descendants, $editCond); $sw.Stop(); $fe += $sw.Elapsed.TotalMilliseconds
  $sw = [Diagnostics.Stopwatch]::StartNew(); $y = $vp.Current.Value; $sw.Stop(); $gv += $sw.Elapsed.TotalMilliseconds
}
Write-Output ("median fullTreeWalk  = {0:N1} ms (nodes={1}, min={2:N1} max={3:N1})" -f (Median $tw), $n, ($tw | Measure-Object -Minimum).Minimum, ($tw | Measure-Object -Maximum).Maximum)
Write-Output ("median findEdit      = {0:N1} ms (min={1:N1} max={2:N1})" -f (Median $fe), ($fe | Measure-Object -Minimum).Minimum, ($fe | Measure-Object -Maximum).Maximum)
Write-Output ("median GetValue      = {0:N2} ms (min={1:N2} max={2:N2})" -f (Median $gv), ($gv | Measure-Object -Minimum).Minimum, ($gv | Measure-Object -Maximum).Maximum)

# ---- SetValue writes Chinese, read back to verify ----
$zh = "写入测试：中文、English、数字 12345、符号！@#￥%……&*（）`r`n第二行 结束"
$sw = [Diagnostics.Stopwatch]::StartNew(); $vp.SetValue($zh); $sw.Stop()
Start-Sleep -Milliseconds 400
$rb = $vp.Current.Value
Write-Output ("SetValue zh: {0:N2} ms  wrote={1} readback={2} equal={3}" -f $sw.Elapsed.TotalMilliseconds, $zh.Length, $rb.Length, ((& $normAll $rb) -eq (& $normAll $zh)))
Write-Output ("  readback raw = {0}" -f $rb.Replace("`r","<CR>").Replace("`n","<LF>"))
$before = (Get-Process -Id $win.Current.ProcessId).WorkingSet64
Write-Output ("notepad WorkingSet = {0:N1} MB" -f ($before / 1MB))

# ---- single-instance / multi-tab verification ----
$tmp2 = Join-Path $env:TEMP "spike-a-second-$stamp.txt"
[IO.File]::WriteAllText($tmp2, "second file`r`n", (New-Object Text.UTF8Encoding($false)))
$p2 = Start-Process notepad -ArgumentList "`"$tmp2`"" -PassThru
Start-Sleep -Seconds 4
$np = @(Get-Process Notepad -ErrorAction SilentlyContinue)
Write-Output ("after opening 2nd file: Notepad proc count={0} pids={1}; 2nd stub pid={2} exited={3}" -f $np.Count, ($np.Id -join ','), $p2.Id, $p2.HasExited)
$tabs = $win.FindAll($TS::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::TabItem)))
Write-Output ("TabItem count in original window = {0}" -f $tabs.Count)
foreach ($t2 in $tabs) { Write-Output ("  tab: '{0}'" -f $t2.Current.Name) }
Get-Process Notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-Item $tmp, $tmp2 -Force -ErrorAction SilentlyContinue
Write-Output "cleanup done"
