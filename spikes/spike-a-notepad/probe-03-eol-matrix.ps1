<#
.SYNOPSIS
  probe-03 -- EOL matrix probe: relation between on-disk EOL and the EOL that UIA returns.

.DESCRIPTION
  Why this probe exists (ADR-0023):
    The first Spike A reconnaissance found "disk file is CRLF, UIA reads back bare CR",
    so a 62-character file read back as 59 characters and the naive assertion
    `value == file content ? False` produced a FALSE NEGATIVE.
    Only one combination (CRLF) had been tested, so three hypotheses were indistinguishable:
      (a) UIA always normalizes paragraph separators to bare CR (RichEdit internal form);
      (b) UIA passes the on-disk EOL through unchanged;
      (c) UIA normalizes only in-line separators and keeps the final one.
    This probe writes four disk shapes (LF / CR / CRLF / MIXED) plus one CJK case,
    reads each back through UIA and prints PER-CHARACTER CODE POINT COUNTS, which
    separates (a)/(b)/(c) in a single run and supplies the measured basis for the
    import/export normalization contract in ADR-0023.

  The status-bar text is collected as well: modern Notepad shows the EOL style IT
  DETECTED (for example " Windows (CRLF)"). That is an independent signal about what
  the application believes the file to be, and it cross-checks the on-disk truth.

.NOTES
  CONSTRAINT (ADR-0022 D1): the PID returned by Start-Process is NEVER used to locate
    the window. This probe locates the window by "unique file-name nonce + enumerate
    top-level Notepad windows + find the TabItem whose Name contains the nonce"; the
    owner PID is read only from the UIA element's ProcessId (equivalent to
    GetWindowThreadProcessId).
  THIS FILE IS PURE ASCII ON PURPOSE (ADR-0024 D4). Windows PowerShell 5.1 decodes a
    BOM-less script as the ANSI code page (GBK on this machine), which corrupts line
    numbers and eats backtick escapes such as `r. The CJK sample below is therefore
    built from code points instead of being written as a literal. The Chinese narrative
    for this probe lives in spikes/spike-a-notepad/README.md and docs/spike-reports/SPIKE-A.md.
  Judge results ONLY from the structured ASCII report this script writes; the console
    may render non-ASCII as garbage. Never assert by eyeballing the console.
  Zero third-party dependencies (only the .NET built-in UIAutomation managed wrapper).
#>

#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$AE   = [System.Windows.Automation.AutomationElement]
$TS   = [System.Windows.Automation.TreeScope]
$CT   = [System.Windows.Automation.ControlType]
$VPat = [System.Windows.Automation.ValuePattern]

$WorkDir = 'D:\csart\eol-probe'
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

# ---------------------------------------------------------------- helpers

function Stop-AllNotepad {
  Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Milliseconds 900
}

function Write-ExactBytes {
  param([string]$Path, [string]$Text)
  # UTF-8 without BOM: a BOM would add one character and pollute the length assertions
  # (this was hit during the reconnaissance run).
  [IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding($false)))
}

function Get-CharCodeSummary {
  param([string]$Text)
  $counts = @{}
  foreach ($ch in $Text.ToCharArray()) {
    $code = [int]$ch
    if ($counts.ContainsKey($code)) { $counts[$code]++ } else { $counts[$code] = 1 }
  }
  # Report only the control characters we care about: 10=LF 13=CR 65279=BOM
  $parts = @()
  foreach ($k in @(10, 13, 65279)) {
    if ($counts.ContainsKey($k)) { $parts += ("code{0}={1}" -f $k, $counts[$k]) }
  }
  if ($parts.Count -eq 0) { return 'none' }
  return ($parts -join ' ')
}

function Find-NotepadWindowByNonce {
  param([string]$Nonce, [int]$TimeoutMs = 20000)
  $root = $AE::RootElement
  $winCond  = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Window)
  $clsCond  = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Notepad')
  $winAll   = New-Object System.Windows.Automation.AndCondition($winCond, $clsCond)
  $tabCond  = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::TabItem)

  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
    $wins = $root.FindAll($TS::Children, $winAll)
    foreach ($w in $wins) {
      $tabs = $w.FindAll($TS::Descendants, $tabCond)
      foreach ($t in $tabs) {
        if ($t.Current.Name -like "*$Nonce*") { return $w }
      }
    }
    Start-Sleep -Milliseconds 250
  }
  throw ("no Notepad window containing nonce '" + $Nonce + "' within " + $TimeoutMs + " ms")
}

function Read-EditorText {
  param($Window)
  $clsCond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'RichEditD2DPT')
  $editor = $Window.FindFirst($TS::Descendants, $clsCond)
  if ($null -eq $editor) { throw 'editor element RichEditD2DPT not found' }
  $vp = $editor.GetCurrentPattern($VPat::Pattern)
  return @{ Text = $vp.Current.Value; Editor = $editor }
}

function Read-StatusBarTexts {
  param($Window)
  $aidCond = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, 'ContentTextBlock')
  $items = $Window.FindAll($TS::Descendants, $aidCond)
  $out = @()
  foreach ($i in $items) { $out += $i.Current.Name }
  return $out
}

# ---------------------------------------------------------------- test matrix

# CJK sample built from code points so that THIS FILE stays pure ASCII (ADR-0024 D4).
#   line 1 = U+4E2D U+6587 U+4E00   line 2 = U+4E2D U+6587 U+4E8C
# Writing them as literals is exactly what corrupted this case on 2026-09-18 before the fix:
# PS 5.1 decoded the BOM-less script as GBK, the literal became 14 chars / 34 bytes with
# only one CR, and norm_eq=True turned into a FALSE POSITIVE (both sides equally mangled).
$CjkLine1 = -join ([char]0x4E2D, [char]0x6587, [char]0x4E00)
$CjkLine2 = -join ([char]0x4E2D, [char]0x6587, [char]0x4E8C)

$Cases = @(
  @{ Name = 'CRLF';      Text = "AAA`r`nBBB`r`nCCC`r`n" },
  @{ Name = 'LF';        Text = "AAA`nBBB`nCCC`n" },
  @{ Name = 'CR';        Text = "AAA`rBBB`rCCC`r" },
  @{ Name = 'MIXED';     Text = "AAA`r`nBBB`nCCC`r" },
  @{ Name = 'CRLF-CJK';  Text = ($CjkLine1 + "`r`n" + $CjkLine2 + "`r`n") }
)

$Results = @()

foreach ($case in $Cases) {
  Stop-AllNotepad
  $nonce = ("eol{0}-{1}" -f $case.Name.ToLower(), [DateTimeOffset]::UtcNow.ToUnixTimeSeconds())
  $file  = Join-Path $WorkDir ("{0}.txt" -f $nonce)
  Write-ExactBytes -Path $file -Text $case.Text
  $diskBytes = (Get-Item $file).Length

  Start-Process notepad -ArgumentList $file | Out-Null
  $win = Find-NotepadWindowByNonce -Nonce $nonce
  $ownerPid = $win.Current.ProcessId
  $read = Read-EditorText -Window $win
  $uia = $read.Text
  $status = Read-StatusBarTexts -Window $win

  # Three-state normalization (the candidate rule of ADR-0023): CRLF -> LF, then lone CR -> LF.
  # ORDER MATTERS and must not be swapped.
  $normDisk = $case.Text.Replace("`r`n", "`n").Replace("`r", "`n")
  $normUia  = $uia.Replace("`r`n", "`n").Replace("`r", "`n")

  $Results += [pscustomobject]@{
    case            = $case.Name
    disk_chars      = $case.Text.Length
    disk_bytes      = $diskBytes
    disk_eol        = (Get-CharCodeSummary -Text $case.Text)
    uia_chars       = $uia.Length
    uia_eol         = (Get-CharCodeSummary -Text $uia)
    roundtrip_raw   = ($uia -ceq $case.Text)
    roundtrip_norm  = ($normUia -ceq $normDisk)
    owner_pid       = $ownerPid
    statusbar_ascii = (($status | Where-Object { $_ -match 'CRLF|LF|CR|Unix|Mac' }) -join '|')
  }

  Stop-AllNotepad
  Remove-Item $file -Force -ErrorAction SilentlyContinue
}

# ---------------------------------------------------------------- output (pure ASCII, avoids console code-page noise)

$report = Join-Path $WorkDir 'RESULT.txt'
$lines = @()
$lines += 'probe-03 EOL matrix - result'
$lines += ('generated_utc=' + [DateTime]::UtcNow.ToString('o'))
$lines += ''
foreach ($r in $Results) {
  $lines += ('case={0,-9} disk_chars={1,-3} disk_bytes={2,-3} uia_chars={3,-3} raw_eq={4,-5} norm_eq={5,-5}' -f `
    $r.case, $r.disk_chars, $r.disk_bytes, $r.uia_chars, $r.roundtrip_raw, $r.roundtrip_norm)
  $lines += ('   disk_eol=[{0}]  uia_eol=[{1}]  owner_pid={2}  statusbar=[{3}]' -f `
    $r.disk_eol, $r.uia_eol, $r.owner_pid, $r.statusbar_ascii)
}
$lines | Set-Content -Path $report -Encoding Ascii
Get-Content $report
Write-Output ''
Write-Output ("report written: " + $report)
