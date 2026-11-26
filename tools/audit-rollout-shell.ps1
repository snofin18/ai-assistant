#requires -Version 7.0

<#
.SYNOPSIS
  Audit Codex rollout JSONL files for exec_command shell selection by turn.

.DESCRIPTION
  Reads an active rollout with FileShare.ReadWrite | Delete, decodes strict UTF-8,
  parses every non-empty JSONL line, and groups exec_command calls by turn_context.

  The tool is deliberately read-only. It does not repair rollout files and does not
  infer shell usage from prose or terminal output.

.PARAMETER RolloutPath
  Rollout JSONL path. Required unless -SelfTest is used.

.PARAMETER LastTurns
  Number of most recent turn_context groups to return. Default 3.

.PARAMETER TurnId
  Exact turn id to return. Takes precedence over LastTurns.

.PARAMETER IncludeCommands
  Include commands for bash, cmd, missing shell, and unknown shell values.

.PARAMETER Format
  Text or Json output. Default Text.

.PARAMETER SelfTest
  Run synthetic fixtures covering CJK, active file sharing, invalid UTF-8, bad JSON,
  and missing turn_context.

.PARAMETER AllowGrowth
  Permit a rollout that grows during read, but mark the snapshot incomplete. Exact
  evidence should be taken after the relevant turn is complete and the file is stable.
#>
[CmdletBinding()]
param(
    [string]$RolloutPath,
    [int]$LastTurns = 3,
    [string]$TurnId,
    [switch]$IncludeCommands,
    [ValidateSet('Text', 'Json')]
    [string]$Format = 'Text',
    [switch]$SelfTest,
    [switch]$AllowGrowth
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

$script:MaxLineCharacters = 16 * 1024 * 1024
$script:MaxTurnCount = 4096
$script:MaxCommandsPerTurn = 4096
$script:MaxShellValuesPerTurn = 64

if (-not ('RolloutLineReaderV1' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

public sealed class RolloutLineReaderV1 : IDisposable
{
    private const int BufferSize = 65536;
    private readonly FileStream stream;
    private readonly UTF8Encoding utf8 = new UTF8Encoding(false, true);
    private readonly byte[] buffer = new byte[BufferSize];
    private readonly int maxLineBytes;
    private int offset;
    private int count;
    private long remaining;
    private long lineNumber;

    public RolloutLineReaderV1(string path, int maximumLineBytes)
    {
        maxLineBytes = maximumLineBytes;
        FileShare share = FileShare.ReadWrite | FileShare.Delete;
        stream = new FileStream(path, FileMode.Open, FileAccess.Read, share, BufferSize, FileOptions.SequentialScan);
        LengthBefore = stream.Length;
        remaining = LengthBefore;
    }

    public long LengthBefore { get; private set; }
    public long LengthAfter { get { return stream.Length; } }
    public long LineNumber { get { return lineNumber; } }

    public bool TryReadLine(out string line, out long physicalLine)
    {
        line = null;
        physicalLine = 0;
        List<byte> bytes = new List<byte>(1024);
        bool sawData = false;
        bool newlineFound = false;

        while (true)
        {
            if (offset >= count)
            {
                if (remaining <= 0)
                {
                    break;
                }
                int requested = (int)Math.Min((long)buffer.Length, remaining);
                int read = stream.Read(buffer, 0, requested);
                if (read <= 0)
                {
                    remaining = 0;
                    break;
                }
                offset = 0;
                count = read;
                remaining -= read;
            }

            sawData = true;
            int available = count - offset;
            int newlineIndex = Array.IndexOf(buffer, (byte)'\n', offset, available);
            int take = newlineIndex >= 0 ? newlineIndex - offset : available;
            int segmentStart = offset;
            offset += take;
            if (newlineIndex >= 0)
            {
                offset += 1;
                newlineFound = true;
            }
            if (bytes.Count + take > maxLineBytes)
            {
                throw new InvalidDataException(
                    "Rollout line " + (lineNumber + 1) + " exceeds the " + maxLineBytes + "-byte line bound");
            }
            for (int index = 0; index < take; index++)
            {
                bytes.Add(buffer[segmentStart + index]);
            }
            if (newlineFound)
            {
                break;
            }
        }

        if (!sawData && !newlineFound && bytes.Count == 0)
        {
            return false;
        }
        if (bytes.Count > 0 && bytes[bytes.Count - 1] == (byte)'\r')
        {
            bytes.RemoveAt(bytes.Count - 1);
        }

        int start = 0;
        if (lineNumber == 0 && bytes.Count >= 3 &&
            bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF)
        {
            start = 3;
        }
        byte[] lineBytes = bytes.GetRange(start, bytes.Count - start).ToArray();
        try
        {
            line = utf8.GetString(lineBytes);
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException(
                "Invalid strict UTF-8 at rollout line " + (lineNumber + 1) + ": " + error.Message,
                error);
        }
        lineNumber += 1;
        physicalLine = lineNumber;
        return true;
    }

    public void Dispose()
    {
        stream.Dispose();
    }
}
'@
}

function Get-OptionalProperty {
    param(
        [AllowNull()]$InputObject,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if ($null -eq $InputObject) {
        return $null
    }
    $property = $InputObject.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Get-SnapshotStatus {
    param(
        [Parameter(Mandatory = $true)][long]$LengthBefore,
        [Parameter(Mandatory = $true)][long]$LengthAfter,
        [switch]$AllowGrowth
    )

    $grew = $LengthAfter -gt $LengthBefore
    if ($grew -and -not $AllowGrowth) {
        throw "Rollout grew during read (before=$LengthBefore after=$LengthAfter); rerun after the turn is complete or use -AllowGrowth for diagnostic output"
    }
    return [pscustomobject]@{
        grew_during_read  = $grew
        snapshot_complete = -not $grew
    }
}

function Assert-StrictJson {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Json,
        [Parameter(Mandatory = $true)]
        [int]$LineNumber,
        [Parameter(Mandatory = $true)]
        [string]$Context
    )

    $document = $null
    try {
        $options = [System.Text.Json.JsonDocumentOptions]::new()
        $options.AllowTrailingCommas = $false
        $options.CommentHandling = [System.Text.Json.JsonCommentHandling]::Disallow
        $document = [System.Text.Json.JsonDocument]::Parse($Json, $options)
    }
    catch {
        throw "Invalid strict JSON in $Context at rollout line ${LineNumber}: $($_.Exception.Message)"
    }
    finally {
        if ($null -ne $document) {
            $document.Dispose()
        }
    }
}

function Convert-ExecArguments {
    param(
        [AllowNull()][string]$ArgumentsJson,
        [Parameter(Mandatory = $true)]
        [int]$LineNumber
    )

    if ([string]::IsNullOrWhiteSpace($ArgumentsJson)) {
        throw "Exec command arguments are empty at rollout line $LineNumber"
    }
    Assert-StrictJson -Json $ArgumentsJson -LineNumber $LineNumber -Context 'exec_command.arguments'
    try {
        return $ArgumentsJson | ConvertFrom-Json
    }
    catch {
        throw "Invalid exec command arguments JSON at rollout line ${LineNumber}: $($_.Exception.Message)"
    }
}

function New-TurnAccumulator {
    param(
        [string]$TurnId,
        [int]$StartLine
    )

    return [ordered]@{
        turn_id      = $TurnId
        start_line   = $StartLine
        powershell   = 0
        bash         = 0
        cmd          = 0
        missing      = 0
        unknown      = 0
        shell_values = [System.Collections.Generic.Dictionary[string, int]]::new([System.StringComparer]::OrdinalIgnoreCase)
        commands     = [System.Collections.Generic.List[object]]::new()
    }
}

function Add-ShellCount {
    param(
        [Parameter(Mandatory = $true)]$Turn,
        [AllowNull()][string]$ShellValue,
        [Parameter(Mandatory = $true)][int]$LineNumber,
        [AllowNull()][string]$Command,
        [switch]$IncludeCommands
    )

    $normalized = if ([string]::IsNullOrWhiteSpace($ShellValue)) {
        '<missing>'
    } else {
        $ShellValue.Trim().ToLowerInvariant()
    }
    if (-not $Turn.shell_values.ContainsKey($normalized) -and $Turn.shell_values.Count -ge $script:MaxShellValuesPerTurn) {
        throw "Turn $($Turn.turn_id) exceeded the shell value bound at rollout line $LineNumber"
    }
    if (-not $Turn.shell_values.ContainsKey($normalized)) {
        $Turn.shell_values[$normalized] = 0
    }
    $Turn.shell_values[$normalized] = $Turn.shell_values[$normalized] + 1

    switch ($normalized) {
        'powershell' {
            $Turn.powershell += 1
        }
        'bash' {
            $Turn.bash += 1
        }
        'cmd' {
            $Turn.cmd += 1
        }
        'cmd.exe' {
            $Turn.cmd += 1
        }
        '<missing>' {
            $Turn.missing += 1
        }
        default {
            $Turn.unknown += 1
        }
    }

    if ($IncludeCommands -and $normalized -ne 'powershell') {
        if ($Turn.commands.Count -ge $script:MaxCommandsPerTurn) {
            throw "Turn $($Turn.turn_id) exceeded the command evidence bound at rollout line $LineNumber"
        }
        $Turn.commands.Add([pscustomobject]@{
            line    = $LineNumber
            shell   = $normalized
            command = $Command
        })
    }
}

function Read-RolloutAudit {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [switch]$IncludeCommands,
        [switch]$AllowGrowth
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $reader = [RolloutLineReaderV1]::new($resolvedPath, $script:MaxLineCharacters)
    $lengthBefore = $reader.LengthBefore
    $turns = [System.Collections.Generic.List[object]]::new()
    $currentTurn = $null
    $lineNumber = 0
    $jsonLineCount = 0
    $orphanExecCalls = 0

    try {
        while ($true) {
            $line = $null
            [long]$physicalLine = 0
            try {
                $hasLine = $reader.TryReadLine([ref]$line, [ref]$physicalLine)
            }
            catch {
                throw $_.Exception.Message
            }
            if (-not $hasLine) {
                break
            }
            $lineNumber = [int]$physicalLine
            if ([string]::IsNullOrWhiteSpace($line)) {
                continue
            }
            Assert-StrictJson -Json $line -LineNumber $lineNumber -Context 'rollout line'
            try {
                $item = $line | ConvertFrom-Json
            }
            catch {
                throw "Validated JSON projection failed at rollout line ${lineNumber}: $($_.Exception.Message)"
            }
            $jsonLineCount += 1

            $itemType = [string](Get-OptionalProperty -InputObject $item -Name 'type')
            if ($itemType -eq 'turn_context') {
                if ($null -ne $currentTurn) {
                    if ($turns.Count -ge $script:MaxTurnCount) {
                        throw "Rollout exceeds the $($script:MaxTurnCount)-segment turn bound at line $lineNumber"
                    }
                    $turns.Add([pscustomobject]$currentTurn)
                }
                $payload = Get-OptionalProperty -InputObject $item -Name 'payload'
                $nextTurnId = [string](Get-OptionalProperty -InputObject $payload -Name 'turn_id')
                if ([string]::IsNullOrWhiteSpace($nextTurnId)) {
                    throw "turn_context has no payload.turn_id at rollout line $lineNumber"
                }
                $currentTurn = New-TurnAccumulator -TurnId $nextTurnId -StartLine $lineNumber
                continue
            }

            if ($itemType -ne 'response_item') {
                continue
            }
            $payload = Get-OptionalProperty -InputObject $item -Name 'payload'
            $payloadType = [string](Get-OptionalProperty -InputObject $payload -Name 'type')
            if ($payloadType -ne 'function_call') {
                continue
            }
            $functionName = [string](Get-OptionalProperty -InputObject $payload -Name 'name')
            if ($functionName -ne 'exec_command') {
                continue
            }
            if ($null -eq $currentTurn) {
                $orphanExecCalls += 1
                throw "exec_command appears before any turn_context at rollout line $lineNumber"
            }

            $argumentsJson = [string](Get-OptionalProperty -InputObject $payload -Name 'arguments')
            $arguments = Convert-ExecArguments -ArgumentsJson $argumentsJson -LineNumber $lineNumber
            $shellValue = [string](Get-OptionalProperty -InputObject $arguments -Name 'shell')
            $commandValue = [string](Get-OptionalProperty -InputObject $arguments -Name 'cmd')
            Add-ShellCount `
                -Turn $currentTurn `
                -ShellValue $shellValue `
                -LineNumber $lineNumber `
                -Command $commandValue `
                -IncludeCommands:$IncludeCommands
        }
    }
    finally {
        $lengthAfter = $reader.LengthAfter
        $reader.Dispose()
    }

    $snapshotStatus = Get-SnapshotStatus -LengthBefore $lengthBefore -LengthAfter $lengthAfter -AllowGrowth:$AllowGrowth
    if ($null -ne $currentTurn) {
        if ($turns.Count -ge $script:MaxTurnCount) {
            throw "Rollout exceeds the $($script:MaxTurnCount)-segment turn bound at final segment"
        }
        $turns.Add([pscustomobject]$currentTurn)
    }
    if ($turns.Count -eq 0) {
        throw "Rollout contains no turn_context boundary: $resolvedPath"
    }

    $logicalTurns = Merge-TurnSegments -Segments $turns
    return [pscustomobject]@{
        schema_version    = 1
        rollout_path      = $resolvedPath
        line_count        = $lineNumber
        json_line_count   = $jsonLineCount
        length_before     = $lengthBefore
        length_after      = $lengthAfter
        grew_during_read  = $snapshotStatus.grew_during_read
        snapshot_complete = $snapshotStatus.snapshot_complete
        turn_count        = $logicalTurns.Count
        orphan_exec_calls = $orphanExecCalls
        turns             = @($logicalTurns)
    }
}

function Merge-TurnSegments {
    param(
        [Parameter(Mandatory = $true)]
        [System.Collections.Generic.List[object]]$Segments
    )

    $merged = [System.Collections.Generic.List[object]]::new()
    $turnIndices = [System.Collections.Generic.Dictionary[string, int]]::new([System.StringComparer]::Ordinal)
    foreach ($segment in $Segments) {
        $turnId = [string]$segment.turn_id
        if (-not $turnIndices.ContainsKey($turnId)) {
            $turnIndices[$turnId] = $merged.Count
            $merged.Add([pscustomobject]@{
                turn_id      = $turnId
                start_line   = [int]$segment.start_line
                powershell   = [int]$segment.powershell
                bash         = [int]$segment.bash
                cmd          = [int]$segment.cmd
                missing      = [int]$segment.missing
                unknown      = [int]$segment.unknown
                shell_values = [System.Collections.Generic.Dictionary[string, int]]::new([System.StringComparer]::OrdinalIgnoreCase)
                commands     = [System.Collections.Generic.List[object]]::new()
            })
            foreach ($pair in $segment.shell_values.GetEnumerator()) {
                $merged[$merged.Count - 1].shell_values[$pair.Key] = $pair.Value
            }
            foreach ($command in $segment.commands) {
                $merged[$merged.Count - 1].commands.Add($command)
            }
            continue
        }

        $target = $merged[$turnIndices[$turnId]]
        $target.start_line = [Math]::Min($target.start_line, [int]$segment.start_line)
        $target.powershell += [int]$segment.powershell
        $target.bash += [int]$segment.bash
        $target.cmd += [int]$segment.cmd
        $target.missing += [int]$segment.missing
        $target.unknown += [int]$segment.unknown
        foreach ($pair in $segment.shell_values.GetEnumerator()) {
            if (-not $target.shell_values.ContainsKey($pair.Key) -and $target.shell_values.Count -ge $script:MaxShellValuesPerTurn) {
                throw "Merged turn $turnId exceeds the shell value bound"
            }
            if (-not $target.shell_values.ContainsKey($pair.Key)) {
                $target.shell_values[$pair.Key] = 0
            }
            $target.shell_values[$pair.Key] = $target.shell_values[$pair.Key] + $pair.Value
        }
        if ($target.commands.Count + $segment.commands.Count -gt $script:MaxCommandsPerTurn) {
            throw "Merged turn $turnId exceeds the command evidence bound"
        }
        foreach ($command in $segment.commands) {
            $target.commands.Add($command)
        }
    }
    return ,$merged
}

function Get-SelectedTurns {
    param(
        [Parameter(Mandatory = $true)]$Audit,
        [string]$RequestedTurnId,
        [Parameter(Mandatory = $true)][int]$TrailingTurns
    )

    if ($TrailingTurns -lt 1) {
        throw 'LastTurns must be at least 1'
    }
    if (-not [string]::IsNullOrWhiteSpace($RequestedTurnId)) {
        $selected = @($Audit.turns | Where-Object { $_.turn_id -eq $RequestedTurnId })
        if ($selected.Count -ne 1) {
            throw "Expected one turn with id $RequestedTurnId, found $($selected.Count)"
        }
        return $selected
    }
    return @($Audit.turns | Select-Object -Last $TrailingTurns)
}

function Get-TurnTotals {
    param(
        [Parameter(Mandatory = $true)]$Turns
    )

    $items = @($Turns)
    return [pscustomobject]@{
        turn_count  = $items.Count
        powershell  = [int](($items | Measure-Object -Property powershell -Sum).Sum)
        bash        = [int](($items | Measure-Object -Property bash -Sum).Sum)
        cmd         = [int](($items | Measure-Object -Property cmd -Sum).Sum)
        missing     = [int](($items | Measure-Object -Property missing -Sum).Sum)
        unknown     = [int](($items | Measure-Object -Property unknown -Sum).Sum)
    }
}

function Format-AuditText {
    param(
        [Parameter(Mandatory = $true)]$Result
    )

    Write-Output "rollout=$($Result.rollout_path)"
    Write-Output "lines=$($Result.line_count) json_lines=$($Result.json_line_count) turns=$($Result.turn_count) grew_during_read=$($Result.grew_during_read) snapshot_complete=$($Result.snapshot_complete)"
    foreach ($turn in $Result.selected_turns) {
        Write-Output "turn=$($turn.turn_id) start_line=$($turn.start_line) is_latest_turn=$($turn.is_latest_turn) turn_complete=$($turn.turn_complete) powershell=$($turn.powershell) bash=$($turn.bash) cmd=$($turn.cmd) missing=$($turn.missing) unknown=$($turn.unknown)"
        foreach ($entry in $turn.commands) {
            Write-Output "  line=$($entry.line) shell=$($entry.shell) command=$($entry.command)"
        }
    }
    Write-Output "selected_total bash=$($Result.selected_total.bash) powershell=$($Result.selected_total.powershell) cmd=$($Result.selected_total.cmd) missing=$($Result.selected_total.missing) unknown=$($Result.selected_total.unknown)"
}

function Assert-Equal {
    param(
        [Parameter(Mandatory = $true)]$Actual,
        [Parameter(Mandatory = $true)]$Expected,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if ($Actual -ne $Expected) {
        throw "SELFTEST FAILED: $Label expected=$Expected actual=$Actual"
    }
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text
    )

    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Invoke-SelfTest {
    $root = Join-Path ([System.IO.Path]::GetTempPath()) ("audit-rollout-shell-{0}-{1}" -f $PID, [Guid]::NewGuid().ToString('N'))
    [System.IO.Directory]::CreateDirectory($root) | Out-Null
    $validPath = Join-Path $root 'valid.jsonl'
    $invalidUtf8Path = Join-Path $root 'invalid-utf8.jsonl'
    $invalidJsonPath = Join-Path $root 'invalid-json.jsonl'
    $missingTurnPath = Join-Path $root 'missing-turn.jsonl'
    $singleTurnPath = Join-Path $root 'single-turn.jsonl'
    $utf16BomPath = Join-Path $root 'utf16-bom.jsonl'
    $invalidUtf8SecondLinePath = Join-Path $root 'invalid-utf8-second-line.jsonl'
    $orphanThenTurnPath = Join-Path $root 'orphan-then-turn.jsonl'
    $lineBoundPath = Join-Path $root 'line-bound.jsonl'

    try {
        $cjk = [string]([char]0x4E2D)
        $turnOneId = 'turn-one'
        $turnTwoId = 'turn-two'
        $lines = [System.Collections.Generic.List[string]]::new()
        $lines.Add(([ordered]@{
            type    = 'turn_context'
            payload = [ordered]@{ turn_id = $turnOneId }
        } | ConvertTo-Json -Compress -Depth 8))
        foreach ($call in @(
            [ordered]@{ shell = 'powershell'; cmd = 'Get-Date' },
            [ordered]@{ shell = 'bash'; cmd = "Write-Output $cjk" },
            [ordered]@{ shell = 'cmd.exe'; cmd = 'dir' }
        )) {
            $lines.Add(([ordered]@{
                type    = 'response_item'
                payload = [ordered]@{
                    type      = 'function_call'
                    name      = 'exec_command'
                    arguments = ($call | ConvertTo-Json -Compress -Depth 8)
                }
            } | ConvertTo-Json -Compress -Depth 8))
        }
        $lines.Add(([ordered]@{
            type    = 'turn_context'
            payload = [ordered]@{ turn_id = $turnOneId }
        } | ConvertTo-Json -Compress -Depth 8))
        $lines.Add(([ordered]@{
            type    = 'response_item'
            payload = [ordered]@{
                type      = 'function_call'
                name      = 'exec_command'
                arguments = ([ordered]@{ shell = 'powershell'; cmd = 'Get-Location' } | ConvertTo-Json -Compress)
            }
        } | ConvertTo-Json -Compress -Depth 8))
        $lines.Add(([ordered]@{
            type    = 'turn_context'
            payload = [ordered]@{ turn_id = $turnTwoId }
        } | ConvertTo-Json -Compress -Depth 8))
        foreach ($call in @(
            [ordered]@{ shell = 'powershell'; cmd = 'Get-ChildItem' },
            [ordered]@{ shell = 'powershell'; cmd = 'git status' },
            [ordered]@{ shell = 'cmd'; cmd = 'dir /b' },
            [ordered]@{ cmd = 'missing-shell' },
            [ordered]@{ shell = 'fish'; cmd = 'unknown-shell' }
        )) {
            $lines.Add(([ordered]@{
                type    = 'response_item'
                payload = [ordered]@{
                    type      = 'function_call'
                    name      = 'exec_command'
                    arguments = ($call | ConvertTo-Json -Compress -Depth 8)
                }
            } | ConvertTo-Json -Compress -Depth 8))
        }
        Write-Utf8NoBom -Path $validPath -Text (($lines -join "`n") + "`n")

        $holdShare = [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete
        $hold = [System.IO.FileStream]::new($validPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, $holdShare)
        try {
            $audit = Read-RolloutAudit -Path $validPath -IncludeCommands
        }
        finally {
            $hold.Dispose()
        }
        Assert-Equal -Actual $audit.turn_count -Expected 2 -Label 'turn_count'
        Assert-Equal -Actual $audit.grew_during_read -Expected $false -Label 'grew_during_read'
        Assert-Equal -Actual $audit.snapshot_complete -Expected $true -Label 'snapshot_complete'
        Assert-Equal -Actual $audit.length_after -Expected $audit.length_before -Label 'stable_length'
        Assert-Equal -Actual $audit.turns[0].bash -Expected 1 -Label 'turn_one_bash'
        Assert-Equal -Actual $audit.turns[0].powershell -Expected 2 -Label 'turn_one_powershell'
        Assert-Equal -Actual $audit.turns[0].cmd -Expected 1 -Label 'turn_one_cmd'
        Assert-Equal -Actual $audit.turns[0].commands.Count -Expected 2 -Label 'turn_one_commands'
        Assert-Equal -Actual $audit.turns[1].missing -Expected 1 -Label 'turn_two_missing'
        Assert-Equal -Actual $audit.turns[1].unknown -Expected 1 -Label 'turn_two_unknown'
        Assert-Equal -Actual $audit.turns[0].commands[0].line -Expected 3 -Label 'bash_command_line'
        if (-not $audit.turns[0].commands[0].command.Contains($cjk)) {
            throw 'SELFTEST FAILED: CJK command did not round-trip'
        }
        $selectedByTurn = @(Get-SelectedTurns -Audit $audit -RequestedTurnId $turnOneId -TrailingTurns 2)
        Assert-Equal -Actual $selectedByTurn.Count -Expected 1 -Label 'turn_id_selection'
        Assert-Equal -Actual $selectedByTurn[0].turn_id -Expected $turnOneId -Label 'turn_id_selection_value'
        $selectedLast = @(Get-SelectedTurns -Audit $audit -RequestedTurnId '' -TrailingTurns 1)
        Assert-Equal -Actual $selectedLast.Count -Expected 1 -Label 'last_turns_selection'
        Assert-Equal -Actual $selectedLast[0].turn_id -Expected $turnTwoId -Label 'last_turns_selection_value'
        $stableSnapshot = Get-SnapshotStatus -LengthBefore 10 -LengthAfter 10
        Assert-Equal -Actual $stableSnapshot.snapshot_complete -Expected $true -Label 'stable_snapshot_complete'
        $growthRejected = $false
        try {
            $null = Get-SnapshotStatus -LengthBefore 10 -LengthAfter 11
        }
        catch {
            $growthRejected = $true
        }
        Assert-Equal -Actual $growthRejected -Expected $true -Label 'growth_rejected'
        $allowedGrowth = Get-SnapshotStatus -LengthBefore 10 -LengthAfter 11 -AllowGrowth
        Assert-Equal -Actual $allowedGrowth.grew_during_read -Expected $true -Label 'allowed_growth_detected'
        Assert-Equal -Actual $allowedGrowth.snapshot_complete -Expected $false -Label 'allowed_growth_incomplete'

        [System.IO.File]::WriteAllBytes(
            $invalidUtf8Path,
            [byte[]](0x7B, 0x22, 0x61, 0x22, 0x3A, 0x22, 0xC3, 0x28, 0x22, 0x7D, 0x0A)
        )
        $invalidUtf8Rejected = $false
        try {
            $null = Read-RolloutAudit -Path $invalidUtf8Path
        }
        catch {
            $invalidUtf8Rejected = $true
        }
        Assert-Equal -Actual $invalidUtf8Rejected -Expected $true -Label 'invalid_utf8_rejected'

        Write-Utf8NoBom -Path $invalidJsonPath -Text "{not-json}`n"
        $invalidJsonRejected = $false
        try {
            $null = Read-RolloutAudit -Path $invalidJsonPath
        }
        catch {
            $invalidJsonRejected = $true
        }
        Assert-Equal -Actual $invalidJsonRejected -Expected $true -Label 'invalid_json_rejected'

        foreach ($laxJson in @(
            '{"type":"turn_context","payload":{"turn_id":"lax"}} // comment',
            '{"type":"turn_context","payload":{"turn_id":"lax"},}',
            "{'type':'turn_context','payload':{'turn_id':'lax'}}",
            '{"type":"turn_context","payload":{"turn_id":NaN}}'
        )) {
            Write-Utf8NoBom -Path $invalidJsonPath -Text ($laxJson + "`n")
            $laxJsonRejected = $false
            try {
                $null = Read-RolloutAudit -Path $invalidJsonPath
            }
            catch {
                $laxJsonRejected = $true
            }
            Assert-Equal -Actual $laxJsonRejected -Expected $true -Label 'lax_json_rejected'
        }

        $singleTurnText = '{"type":"turn_context","payload":{"turn_id":"single"}}' + "`n"
        Write-Utf8NoBom -Path $singleTurnPath -Text $singleTurnText
        $singleTurnAudit = Read-RolloutAudit -Path $singleTurnPath
        Assert-Equal -Actual $singleTurnAudit.turn_count -Expected 1 -Label 'single_turn_count'
        Assert-Equal -Actual $singleTurnAudit.turns[0].turn_id -Expected 'single' -Label 'single_turn_id'
        Write-Utf8NoBom -Path $singleTurnPath -Text $singleTurnText.TrimEnd("`n")
        $singleTurnNoLfAudit = Read-RolloutAudit -Path $singleTurnPath
        Assert-Equal -Actual $singleTurnNoLfAudit.turn_count -Expected 1 -Label 'single_turn_without_final_lf'

        $oldMaxTurnCount = $script:MaxTurnCount
        $script:MaxTurnCount = 1
        try {
            Write-Utf8NoBom -Path $singleTurnPath -Text ($singleTurnText + $singleTurnText)
            $turnBoundRejected = $false
            try {
                $null = Read-RolloutAudit -Path $singleTurnPath
            }
            catch {
                $turnBoundRejected = $true
            }
            Assert-Equal -Actual $turnBoundRejected -Expected $true -Label 'turn_bound_rejected'
        }
        finally {
            $script:MaxTurnCount = $oldMaxTurnCount
        }

        $utf16Body = [System.Text.Encoding]::Unicode.GetBytes($singleTurnText)
        $utf16Preamble = [System.Text.Encoding]::Unicode.GetPreamble()
        $utf16Bytes = [byte[]]::new($utf16Preamble.Length + $utf16Body.Length)
        [Array]::Copy($utf16Preamble, 0, $utf16Bytes, 0, $utf16Preamble.Length)
        [Array]::Copy($utf16Body, 0, $utf16Bytes, $utf16Preamble.Length, $utf16Body.Length)
        [System.IO.File]::WriteAllBytes($utf16BomPath, $utf16Bytes)
        $utf16Rejected = $false
        try {
            $null = Read-RolloutAudit -Path $utf16BomPath
        }
        catch {
            $utf16Rejected = $true
        }
        Assert-Equal -Actual $utf16Rejected -Expected $true -Label 'utf16_bom_rejected'

        $firstUtf8Line = [System.Text.Encoding]::UTF8.GetBytes($singleTurnText)
        $invalidSecondLine = [byte[]](0xC3, 0x28, 0x0A)
        $invalidSecondBytes = [byte[]]::new($firstUtf8Line.Length + $invalidSecondLine.Length)
        [Array]::Copy($firstUtf8Line, 0, $invalidSecondBytes, 0, $firstUtf8Line.Length)
        [Array]::Copy($invalidSecondLine, 0, $invalidSecondBytes, $firstUtf8Line.Length, $invalidSecondLine.Length)
        [System.IO.File]::WriteAllBytes($invalidUtf8SecondLinePath, $invalidSecondBytes)
        $invalidUtf8LineMessage = ''
        try {
            $null = Read-RolloutAudit -Path $invalidUtf8SecondLinePath
        }
        catch {
            $invalidUtf8LineMessage = $_.Exception.Message
        }
        if (-not $invalidUtf8LineMessage.Contains('rollout line 2')) {
            throw "SELFTEST FAILED: invalid UTF-8 line number missing from '$invalidUtf8LineMessage'"
        }

        $orphanLine = [ordered]@{
            type    = 'response_item'
            payload = [ordered]@{
                type      = 'function_call'
                name      = 'exec_command'
                arguments = ([ordered]@{ shell = 'bash'; cmd = 'pwd' } | ConvertTo-Json -Compress)
            }
        } | ConvertTo-Json -Compress -Depth 8
        Write-Utf8NoBom -Path $missingTurnPath -Text ($orphanLine + "`n")
        $missingTurnRejected = $false
        try {
            $null = Read-RolloutAudit -Path $missingTurnPath
        }
        catch {
            $missingTurnRejected = $true
        }
        Assert-Equal -Actual $missingTurnRejected -Expected $true -Label 'missing_turn_rejected'

        Write-Utf8NoBom -Path $orphanThenTurnPath -Text ($orphanLine + "`n" + $singleTurnText)
        $orphanThenTurnRejected = $false
        try {
            $null = Read-RolloutAudit -Path $orphanThenTurnPath
        }
        catch {
            $orphanThenTurnRejected = $true
        }
        Assert-Equal -Actual $orphanThenTurnRejected -Expected $true -Label 'orphan_then_turn_rejected'

        Write-Utf8NoBom -Path $lineBoundPath -Text ("x" * 32 + "`n")
        $lineBoundReader = [RolloutLineReaderV1]::new($lineBoundPath, 16)
        $lineBoundRejected = $false
        try {
            $boundedLine = $null
            [long]$boundedLineNumber = 0
            $null = $lineBoundReader.TryReadLine([ref]$boundedLine, [ref]$boundedLineNumber)
        }
        catch {
            $lineBoundRejected = $true
        }
        finally {
            $lineBoundReader.Dispose()
        }
        Assert-Equal -Actual $lineBoundRejected -Expected $true -Label 'line_bound_rejected'

        $firstCommands = [System.Collections.Generic.List[object]]::new()
        $firstCommands.Add([pscustomobject]@{ line = 2; shell = 'bash'; command = 'a' })
        $firstCommands.Add([pscustomobject]@{ line = 3; shell = 'bash'; command = 'b' })
        $secondCommands = [System.Collections.Generic.List[object]]::new()
        $secondCommands.Add([pscustomobject]@{ line = 5; shell = 'bash'; command = 'c' })
        $secondCommands.Add([pscustomobject]@{ line = 6; shell = 'bash'; command = 'd' })
        $firstShells = [System.Collections.Generic.Dictionary[string, int]]::new([System.StringComparer]::OrdinalIgnoreCase)
        $firstShells['a'] = 1
        $firstShells['b'] = 1
        $secondShells = [System.Collections.Generic.Dictionary[string, int]]::new([System.StringComparer]::OrdinalIgnoreCase)
        $secondShells['c'] = 1
        $secondShells['d'] = 1
        $mergeSegments = [System.Collections.Generic.List[object]]::new()
        $mergeSegments.Add([pscustomobject]@{
            turn_id = 'merge-bound'; start_line = 1; powershell = 0; bash = 2; cmd = 0; missing = 0; unknown = 0
            shell_values = $firstShells; commands = $firstCommands
        })
        $mergeSegments.Add([pscustomobject]@{
            turn_id = 'merge-bound'; start_line = 4; powershell = 0; bash = 2; cmd = 0; missing = 0; unknown = 0
            shell_values = $secondShells; commands = $secondCommands
        })

        $oldMaxShellValues = $script:MaxShellValuesPerTurn
        $script:MaxShellValuesPerTurn = 2
        try {
            $mergedShellBoundRejected = $false
            try {
                $null = Merge-TurnSegments -Segments $mergeSegments
            }
            catch {
                $mergedShellBoundRejected = $true
            }
            Assert-Equal -Actual $mergedShellBoundRejected -Expected $true -Label 'merged_shell_bound_rejected'
        }
        finally {
            $script:MaxShellValuesPerTurn = $oldMaxShellValues
        }

        $oldMaxCommands = $script:MaxCommandsPerTurn
        $script:MaxCommandsPerTurn = 2
        try {
            $mergedCommandBoundRejected = $false
            try {
                $null = Merge-TurnSegments -Segments $mergeSegments
            }
            catch {
                $mergedCommandBoundRejected = $true
            }
            Assert-Equal -Actual $mergedCommandBoundRejected -Expected $true -Label 'merged_command_bound_rejected'
        }
        finally {
            $script:MaxCommandsPerTurn = $oldMaxCommands
        }

        Write-Output 'SELFTEST PASSED'
    }
    finally {
        if ([System.IO.Directory]::Exists($root)) {
            [System.IO.Directory]::Delete($root, $true)
        }
    }
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if ([string]::IsNullOrWhiteSpace($RolloutPath)) {
    throw 'RolloutPath is required unless -SelfTest is used'
}

$audit = Read-RolloutAudit -Path $RolloutPath -IncludeCommands:$IncludeCommands -AllowGrowth:$AllowGrowth
$selectedTurns = Get-SelectedTurns -Audit $audit -RequestedTurnId $TurnId -TrailingTurns $LastTurns
$lastTurnId = $audit.turns[$audit.turns.Count - 1].turn_id
foreach ($turn in $selectedTurns) {
    $isLatestTurn = $turn.turn_id -eq $lastTurnId
    $turn | Add-Member -NotePropertyName is_latest_turn -NotePropertyValue $isLatestTurn -Force
    $turn | Add-Member -NotePropertyName turn_complete -NotePropertyValue ((-not $isLatestTurn) -or $audit.snapshot_complete) -Force
}
$selectedTotal = Get-TurnTotals -Turns $selectedTurns
$allTotal = Get-TurnTotals -Turns $audit.turns
$selectedSnapshotComplete = -not @($selectedTurns | Where-Object { -not $_.turn_complete }).Count
$result = [pscustomobject]@{
    schema_version    = 1
    rollout_path      = $audit.rollout_path
    line_count        = $audit.line_count
    json_line_count   = $audit.json_line_count
    length_before     = $audit.length_before
    length_after      = $audit.length_after
    grew_during_read  = $audit.grew_during_read
    snapshot_complete = $audit.snapshot_complete
    selected_snapshot_complete = $selectedSnapshotComplete
    turn_count        = $audit.turn_count
    orphan_exec_calls = $audit.orphan_exec_calls
    selected_total    = $selectedTotal
    all_total         = $allTotal
    selected_turns    = @($selectedTurns)
}

if ($Format -eq 'Json') {
    $result | ConvertTo-Json -Depth 12
}
else {
    Format-AuditText -Result $result
}
