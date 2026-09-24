<#
.SYNOPSIS
  probe-04: write-path EOL behaviour. What separator should SetValue use, and what
  ends up on disk after Notepad saves?

.DESCRIPTION
  probe-03 proved the READ direction: UIA always returns bare CR as the paragraph
  separator, regardless of the on-disk EOL (LF / CR / CRLF / mixed all came back as CR).
  This probe covers the WRITE direction and answers the two questions ADR-0023 must answer:

    Q1  After SetValue with LF-separated text, how do the editor and the status bar report it?
    Q2  After saving, what is the on-disk EOL - does it follow what we wrote, or does Notepad
        preserve the file's original EOL style?

  Q2 decides whether "normalize at the boundary" silently destroys the file's line-ending
  style. If it does, the original EOL must be captured separately and restored on write-back.

  ENCODING RULE (learned the hard way; see docs/memory/pitfalls.md):
    Windows PowerShell 5.1 reads a BOM-less .ps1 file as ANSI (this machine: CP936/GBK).
    Any non-ASCII byte sequence - Chinese text, em dash, arrow - can be mis-decoded and can
    swallow a following structural byte, producing a parse error whose reported line number
    does not match the file. Therefore THIS FILE IS PURE ASCII. Non-ASCII test data is built
    from code points ([char]0x4E2D) instead of literals.

.NOTES
  Saving uses Ctrl+S (L4 synthetic keyboard) because the goal is to observe Notepad's own
  save behaviour, not to exercise UIA menu invocation. Ctrl+S is not affected by the IME.
  Window location follows ADR-0022 D1: the PID returned by Start-Process is never used.
#>

#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
# TASK-101: Win32 Input helper (replaces deprecated SendKeys)
Import-Module (Join-Path $PSScriptRoot 'Win32-Input.psm1') -Force -ErrorAction Stop

$AE   = [System.Windows.Automation.AutomationElement]
$TS   = [System.Windows.Automation.TreeScope]
$CT   = [System.Windows.Automation.ControlType]
$VPat = [System.Windows.Automation.ValuePattern]

$WorkDir = 'D:\csart\eol-probe'
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

# CJK sample built from code points so that this script stays pure ASCII.
$Cjk = -join @([char]0x4E2D, [char]0x6587, [char]0x4E00, [char]0x0041, [char]0x4E2D, [char]0x6587, [char]0x4E8C)

# Single-line member definition on purpose: a nested here-string inside this file would
# terminate the outer literal used by the generator that writes this script.
$FgDefs = '[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);'

function Stop-AllNotepad {
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Milliseconds 900
}

function Write-Exact {
  param([string]$Path, [string]$Text)
  [IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Get-EolSummary {
  param([string]$Text)
  $cr = ([regex]::Matches($Text, "`r")).Count
  $lf = ([regex]::Matches($Text, "`n")).Count
  return ('CR=' + $cr + ' LF=' + $lf)
}

function Find-NotepadWindow {
  param([string]$Nonce, [int]$TimeoutMs = 20000)
  $root = $AE::RootElement
  $c = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Window)),
        (New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Notepad')))
  $tc = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::TabItem)
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
    foreach ($w in $root.FindAll($TS::Children, $c)) {
      foreach ($t in $w.FindAll($TS::Descendants, $tc)) {
        if ($t.Current.Name -like ('*' + $Nonce + '*')) { return $w }
      }
    }
    Start-Sleep -Milliseconds 250
  }
  throw ('window not found for nonce ' + $Nonce)
}

function Get-EditorElement {
  param($Window)
  $c = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'RichEditD2DPT')
  $e = $Window.FindFirst($TS::Descendants, $c)
  if ($null -eq $e) { throw 'editor RichEditD2DPT not found' }
  return $e
}

function Get-StatusBarEol {
  param($Window)
  $c = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, 'ContentTextBlock')
  $out = @()
  foreach ($i in $Window.FindAll($TS::Descendants, $c)) { $out += $i.Current.Name }
  return (($out | Where-Object { $_ -match 'CRLF|LF|CR' }) -join '|')
}

function Invoke-SaveWithCtrlS {
  param($Window)
  $hwnd = [IntPtr]$Window.Current.NativeWindowHandle
  if (-not ('Probe.Win32Fg' -as [type])) {
    Add-Type -Name Win32Fg -Namespace Probe -MemberDefinition $FgDefs
  }
  if ([Probe.Win32Fg]::IsIconic($hwnd)) { return 'MINIMIZED' }
  [Probe.Win32Fg]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 600
  # TASK-101: replaced deprecated SendKeys with Win32 SendInput
  Send-SendInputVk -Vk 0x53 -Modifier @(0xA2) | Out-Null  # Ctrl+S
  Start-Sleep -Milliseconds 1800
  return 'SAVED'
}

# disk = the file's original EOL style; write = the separator we feed to SetValue
$Cases = @(
  @{ Name = 'disk=CRLF_write=LF';   Disk = "AAA`r`nBBB`r`n"; New = ("XXX`nYYY`n" + $Cjk) },
  @{ Name = 'disk=CRLF_write=CRLF'; Disk = "AAA`r`nBBB`r`n"; New = "XXX`r`nYYY`r`n" },
  @{ Name = 'disk=CRLF_write=CR';   Disk = "AAA`r`nBBB`r`n"; New = "XXX`rYYY`r" },
  @{ Name = 'disk=LF_write=LF';     Disk = "AAA`nBBB`n";     New = "XXX`nYYY`n" },
  @{ Name = 'disk=LF_write=CRLF';   Disk = "AAA`nBBB`n";     New = "XXX`r`nYYY`r`n" }
)

$lines = @('probe-04 write-path EOL - result', ('generated_utc=' + [DateTime]::UtcNow.ToString('o')), '')

foreach ($case in $Cases) {
  Stop-AllNotepad
  $nonce = 'wp' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
  $file  = Join-Path $WorkDir ($nonce + '.txt')
  Write-Exact -Path $file -Text $case.Disk
  $diskBefore = [IO.File]::ReadAllText($file)

  Start-Process notepad -ArgumentList $file | Out-Null
  $win = Find-NotepadWindow -Nonce $nonce
  $ed  = Get-EditorElement -Window $win
  $vp  = $ed.GetCurrentPattern($VPat::Pattern)

  $statusBefore = Get-StatusBarEol -Window $win
  $vp.SetValue($case.New)
  Start-Sleep -Milliseconds 500
  $afterSet      = $vp.Current.Value
  $statusAfter   = Get-StatusBarEol -Window $win
  $saveResult    = Invoke-SaveWithCtrlS -Window $win
  $diskAfter     = [IO.File]::ReadAllText($file)

  $normNew  = $case.New.Replace("`r`n", "`n").Replace("`r", "`n")
  $normSet  = $afterSet.Replace("`r`n", "`n").Replace("`r", "`n")
  $normDisk = $diskAfter.Replace("`r`n", "`n").Replace("`r", "`n")

  $lines += ('CASE ' + $case.Name)
  $lines += ('   write_input    : ' + (Get-EolSummary $case.New) + '   chars=' + $case.New.Length)
  $lines += ('   uia_after_set  : ' + (Get-EolSummary $afterSet) + '   chars=' + $afterSet.Length)
  $lines += ('   disk_before    : ' + (Get-EolSummary $diskBefore) + '   statusbar=[' + $statusBefore + ']')
  $lines += ('   disk_after     : ' + (Get-EolSummary $diskAfter) + '   statusbar=[' + $statusAfter + ']  save=' + $saveResult)
  $lines += ('   ASSERT uia_equals_write_normalized   : ' + ($normSet -ceq $normNew))
  $lines += ('   ASSERT disk_equals_write_normalized  : ' + ($normDisk -ceq $normNew))
  $lines += ('   ASSERT disk_eol_style_preserved      : ' + ((Get-EolSummary $diskAfter) -eq (Get-EolSummary $diskBefore)))
  $lines += ('   ASSERT cjk_survived_on_disk          : ' + $diskAfter.Contains([string][char]0x4E2D))
  $lines += ('   owner_pid=' + $win.Current.ProcessId)
  $lines += ''

  Stop-AllNotepad
  Remove-Item $file -Force -ErrorAction SilentlyContinue
}

$report = Join-Path $WorkDir 'RESULT-04.txt'
$lines | Set-Content -Path $report -Encoding Ascii
Get-Content $report
