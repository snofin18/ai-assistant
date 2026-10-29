$ErrorActionPreference = 'Stop'

function Write-HookDecision {
    param(
        [string]$Decision,
        [string]$Reason
    )

    $result = [ordered]@{
        hookSpecificOutput = [ordered]@{
            hookEventName = 'PreToolUse'
            permissionDecision = $Decision
            permissionDecisionReason = $Reason
        }
    }
    $json = $result | ConvertTo-Json -Compress -Depth 4
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
    [Console]::Out.Write($json)
}

try {
    $payloadText = [Console]::In.ReadToEnd()
    # The hook matcher already limits execution to Bash. Keep the same fast
    # path inside the script so a directly invoked or misconfigured hook never
    # parses unrelated MCP payloads whose strings can exceed the host bridge's
    # stable JSON forwarding limit.
    if ($payloadText -notmatch '"tool_name"\s*:\s*"Bash"') {
        exit 0
    }
    $payload = $payloadText | ConvertFrom-Json
    if ($payload.tool_name -ne 'Bash') {
        exit 0
    }

    $command = [string]$payload.tool_input.command
    if ([string]::IsNullOrWhiteSpace($command)) {
        throw 'empty shell command'
    }
    if ($command.Length -gt 10000) {
        throw "command length $($command.Length) exceeds 10000"
    }

    # The host does not expose the selected shell to PreToolUse. Rewrite every
    # shell command to an explicit Windows PowerShell process so a mistaken
    # shell=bash selection cannot execute the command in WSL.
    $encodedCommand = [Convert]::ToBase64String(
        [System.Text.Encoding]::Unicode.GetBytes(
            "`$ProgressPreference = 'SilentlyContinue'; $command"
        )
    )
    $wrappedCommand = "powershell.exe -NoLogo -NoProfile -NonInteractive -OutputFormat Text -ExecutionPolicy Bypass -EncodedCommand $encodedCommand"

    $result = [ordered]@{
        hookSpecificOutput = [ordered]@{
            hookEventName = 'PreToolUse'
            permissionDecision = 'allow'
            updatedInput = [ordered]@{
                command = $wrappedCommand
            }
        }
    }
    $json = $result | ConvertTo-Json -Compress -Depth 6
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
    [Console]::Out.Write($json)
} catch {
    $reason = "PowerShell guard failed closed: $($_.Exception.Message)"
    if ($reason.Length -gt 500) {
        $reason = $reason.Substring(0, 500)
    }
    Write-HookDecision -Decision 'deny' -Reason $reason
    exit 0
}
