param(
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [string]$EdgeExecutable = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    [string]$ProfileRoot = "$env:LOCALAPPDATA\ai-assistant-task-364-edge-profile",
    [string]$AgentCoreBinary = "target\debug\assistant-agent-core.exe",
    [string]$DesktopUiBinary = "apps\desktop-ui\src-tauri\target\debug\assistant-desktop-ui.exe",
    [int]$Port = 8766,
    [int]$UiDecisionTimeoutSeconds = 300
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
$agentCorePath = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $AgentCoreBinary))
$desktopUiPath = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $DesktopUiBinary))
$edgePath = [System.IO.Path]::GetFullPath($EdgeExecutable)
$profilePath = [System.IO.Path]::GetFullPath($ProfileRoot)
$packagePath = Join-Path $repoRoot "adapters\browser.edge\tasks\t5.2.profile-human-approval.json"
$fixtureRoot = Join-Path $PSScriptRoot "fixtures"
$serverDir = Join-Path $evidence "server"
$coreDir = Join-Path $evidence "core"
$uiDir = Join-Path $evidence "ui"
$runDir = Join-Path $evidence "human"

if ($Port -lt 1 -or $Port -gt 65535) {
    throw "Port must be in 1..65535"
}
if ($UiDecisionTimeoutSeconds -lt 30 -or $UiDecisionTimeoutSeconds -gt 1800) {
    throw "UiDecisionTimeoutSeconds must be in 30..1800"
}
foreach ($required in @($agentCorePath, $desktopUiPath, $edgePath, $packagePath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required file does not exist: $required"
    }
}
if (-not (Test-Path -LiteralPath $profilePath -PathType Container)) {
    New-Item -ItemType Directory -Path $profilePath | Out-Null
}
New-Item -ItemType Directory -Force -Path $serverDir, $coreDir, $uiDir, $runDir | Out-Null

$jsonPath = Join-Path $runDir "result.json"
$summaryPath = Join-Path $evidence "summary.json"
if (Test-Path -LiteralPath $jsonPath) {
    throw "Result already exists: $jsonPath"
}
if (Test-Path -LiteralPath $summaryPath) {
    throw "Summary already exists: $summaryPath"
}

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text
    )
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Text, $encoding)
}

function Write-JsonFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )
    $text = (($Value | ConvertTo-Json -Depth 10) + "`n") -replace "`r`n", "`n"
    Write-Utf8NoBom -Path $Path -Text $text
}

function Write-Phase {
    param([Parameter(Mandatory = $true)][string]$Phase)
    Write-Utf8NoBom -Path (Join-Path $evidence "phase.txt") -Text "$Phase`n"
}

function Wait-FixtureServer {
    param([Parameter(Mandatory = $true)][string]$Url)
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        try {
            $response = Invoke-WebRequest -UseBasicParsing -Uri $Url -TimeoutSec 2
            if ($response.StatusCode -eq 200) {
                return $response
            }
        } catch {
        }
        Start-Sleep -Milliseconds 250
    }
    throw "Fixture server did not become ready: $Url"
}

function Wait-NamedPipe {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)]$CoreProcess
    )
    for ($attempt = 0; $attempt -lt 240; $attempt++) {
        if ($CoreProcess.HasExited) {
            throw "Core exited before opening UI pipe $Name"
        }
        $pipe = Get-ChildItem -LiteralPath "\\.\pipe\" -ErrorAction SilentlyContinue |
            Where-Object { $_.PSChildName -eq $Name } |
            Select-Object -First 1
        if ($pipe) {
            return
        }
        Start-Sleep -Milliseconds 250
    }
    throw "Core did not open UI pipe $Name within 60 seconds"
}

function Wait-EdgeProfileGone {
    param([Parameter(Mandatory = $true)][string]$Profile)
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        $processes = @(
            Get-CimInstance Win32_Process -Filter "Name='msedge.exe'" |
                Where-Object { $_.CommandLine -like "*$Profile*" }
        )
        if ($processes.Count -eq 0) {
            return
        }
        Start-Sleep -Milliseconds 250
    }
    throw "Dedicated Edge process tree did not converge for $Profile"
}

function Restore-EnvironmentValue {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowNull()][string]$Value
    )
    if ($null -eq $Value) {
        Remove-Item -LiteralPath "Env:$Name" -ErrorAction SilentlyContinue
    } else {
        Set-Item -LiteralPath "Env:$Name" -Value $Value
    }
}

function ConvertTo-StartProcessArguments {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    return @(
        $Arguments | ForEach-Object {
            if ($_ -match "\s") {
                '"' + $_ + '"'
            } else {
                $_
            }
        }
    )
}

$serverProcess = $null
$coreProcess = $null
$uiProcess = $null
$failure = $null
$startedAt = (Get-Date).ToUniversalTime().ToString("o")
$edgeVersion = (Get-Item -LiteralPath $edgePath).VersionInfo.ProductVersion
$tokenVariableName = "ASSISTANT_AGENT_CORE_UI_TOKEN"
$pipeName = "assistant-agent-core-ui-task364-$([Guid]::NewGuid().ToString('N'))"
$previousToken = $env:ASSISTANT_AGENT_CORE_UI_TOKEN
$previousPipe = $env:ASSISTANT_UI_PIPE
$previousTokenVariable = $env:ASSISTANT_UI_TOKEN_ENV

try {
    $listener = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    if ($listener) {
        throw "Port $Port already has a listener"
    }

    $tokenBytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($tokenBytes)
    $uiToken = [Convert]::ToHexString($tokenBytes).ToLowerInvariant()
    $env:ASSISTANT_AGENT_CORE_UI_TOKEN = $uiToken
    $env:ASSISTANT_UI_PIPE = $pipeName
    $env:ASSISTANT_UI_TOKEN_ENV = $tokenVariableName

    $python = (Get-Command python).Source
    $generateOutput = & $python (Join-Path $PSScriptRoot "generate_fixtures.py") 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Fixture generation failed: $generateOutput"
    }
    Write-Utf8NoBom -Path (Join-Path $evidence "fixture-manifest.stdout.log") -Text (
        ($generateOutput -join "`n") + "`n"
    )

    $serverStdout = Join-Path $serverDir "stdout.log"
    $serverStderr = Join-Path $serverDir "stderr.log"
    $serverScript = Join-Path $PSScriptRoot "serve_fixture.py"
    $serverProcess = Start-Process -FilePath $python `
        -ArgumentList @($serverScript, "--root", $fixtureRoot, "--port", "$Port") `
        -WorkingDirectory $repoRoot -WindowStyle Hidden `
        -RedirectStandardOutput $serverStdout `
        -RedirectStandardError $serverStderr `
        -PassThru
    Write-Utf8NoBom -Path (Join-Path $serverDir "pid.txt") -Text "$($serverProcess.Id)`n"
    $health = Wait-FixtureServer -Url "http://127.0.0.1:$Port/form.html"
    Write-Utf8NoBom -Path (Join-Path $serverDir "health.txt") -Text (
        "fixture_server_pid=$($serverProcess.Id)`n" +
        "status=$($health.StatusCode)`n" +
        "bytes=$($health.RawContentLength)`n"
    )

    $profileId = "task364-ui-human-$([Guid]::NewGuid().ToString('N').Substring(0, 8))"
    $runProfile = Join-Path $profilePath $profileId
    $coreArguments = @(
        "--browser-task-run",
        "--task-package",
        $packagePath,
        "--output", $jsonPath,
        "--ui-approval",
        "--ui-pipe", $pipeName,
        "--ui-token-env", $tokenVariableName,
        "--ui-peer", $desktopUiPath,
        "--approve-step", "navigate_form",
        "--approve-step", "fill_form",
        "--browser", "edge",
        "--browser-executable", $edgePath,
        "--browser-profile-root", $profilePath,
        "--browser-profile-id", $profileId,
        "--browser-session-timeout-ms", "180000"
    )
    Write-Utf8NoBom -Path (Join-Path $coreDir "command.txt") -Text ($coreArguments -join "`n")
    $coreLaunchArguments = ConvertTo-StartProcessArguments -Arguments $coreArguments
    $coreProcess = Start-Process -FilePath $agentCorePath `
        -ArgumentList $coreLaunchArguments `
        -WorkingDirectory $repoRoot -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $coreDir "stdout.log") `
        -RedirectStandardError (Join-Path $coreDir "stderr.log") `
        -PassThru
    Write-Utf8NoBom -Path (Join-Path $coreDir "pid.txt") -Text "$($coreProcess.Id)`n"
    Write-Phase -Phase "waiting_for_core_pipe"
    Wait-NamedPipe -Name $pipeName -CoreProcess $coreProcess

    $uiProcess = Start-Process -FilePath $desktopUiPath `
        -WorkingDirectory $repoRoot `
        -RedirectStandardOutput (Join-Path $uiDir "stdout.log") `
        -RedirectStandardError (Join-Path $uiDir "stderr.log") `
        -PassThru
    Write-Utf8NoBom -Path (Join-Path $uiDir "pid.txt") -Text "$($uiProcess.Id)`n"
    Write-Phase -Phase "awaiting_human_confirmation"
    Write-Utf8NoBom -Path (Join-Path $runDir "pre-approval.txt") -Text (
        "result_exists=$([bool](Test-Path -LiteralPath $jsonPath))`n" +
        "pipe=$pipeName`n" +
        "ui_pid=$($uiProcess.Id)`n" +
        "core_pid=$($coreProcess.Id)`n"
    )

    $deadline = (Get-Date).AddSeconds($UiDecisionTimeoutSeconds)
    while (-not (Test-Path -LiteralPath $jsonPath -PathType Leaf)) {
        if ($coreProcess.HasExited) {
            throw "Core exited before human approval result was written"
        }
        if ($uiProcess.HasExited) {
            throw "desktop-ui exited before a human decision"
        }
        if ((Get-Date) -ge $deadline) {
            throw "No human decision was received within $UiDecisionTimeoutSeconds seconds"
        }
        Start-Sleep -Milliseconds 250
    }
    Write-Phase -Phase "human_decision_received"

    if (-not $coreProcess.WaitForExit(30000)) {
        throw "Core did not exit within 30 seconds after writing result JSON"
    }
    if ($coreProcess.ExitCode -ne 0) {
        throw "Core exited with code $($coreProcess.ExitCode)"
    }
    $result = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
    if ($result.status -ne "completed" -or @($result.steps).Count -ne 4) {
        throw "Result status or step count is invalid"
    }
    $submitSteps = @($result.steps | Where-Object { $_.id -eq "submit_form" })
    if ($submitSteps.Count -ne 1 -or $submitSteps[0].status -ne "committed") {
        throw "Result must contain exactly one committed submit_form step"
    }
    if ($result.outputs.submitted -ne $true) {
        throw "Result does not report submitted=true"
    }
    $expectedPostUrl = "http://127.0.0.1:$Port/t5.2/submitted"
    if ([string]$result.outputs.post_url -ne $expectedPostUrl) {
        throw "Result post_url is not the query-free local success page"
    }
    $success = Invoke-WebRequest -UseBasicParsing -Uri $expectedPostUrl -TimeoutSec 5
    if ($success.StatusCode -ne 200 -or $success.Content -notmatch "submitted-marker") {
        throw "Success page marker was not observed"
    }
    Wait-EdgeProfileGone -Profile $runProfile

    $summary = [pscustomobject]@{
        schema_version = 1
        mode = "desktop-ui-human-approval-local"
        result = "passed"
        task_package = $packagePath
        agent_core_binary = $agentCorePath
        desktop_ui_binary = $desktopUiPath
        edge_executable = $edgePath
        edge_version = $edgeVersion
        profile_root = $profilePath
        profile_id = $profileId
        output_json = $jsonPath
        ui_pipe = $pipeName
        core_pid = $coreProcess.Id
        ui_pid = $uiProcess.Id
        started_at_utc = $startedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        pre_approval_result_exists = $false
        submit_step_count = $submitSteps.Count
        submitted = $result.outputs.submitted
        post_url = $result.outputs.post_url
        submitted_marker = "submitted-marker"
        token_logged = $false
    }
    Write-JsonFile -Path $summaryPath -Value $summary
    Write-Phase -Phase "passed"
} catch {
    $failure = $_.Exception.Message
} finally {
    if ($uiProcess -ne $null -and -not $uiProcess.HasExited) {
        try {
            Stop-Process -Id $uiProcess.Id -Force -ErrorAction SilentlyContinue
            Wait-Process -Id $uiProcess.Id -Timeout 10 -ErrorAction SilentlyContinue
        } catch {
            if ($failure -eq $null) {
                $failure = "desktop-ui cleanup failed: $($_.Exception.Message)"
            }
        }
    }
    if ($coreProcess -ne $null -and -not $coreProcess.HasExited) {
        try {
            Stop-Process -Id $coreProcess.Id -Force -ErrorAction SilentlyContinue
            Wait-Process -Id $coreProcess.Id -Timeout 10 -ErrorAction SilentlyContinue
        } catch {
            if ($failure -eq $null) {
                $failure = "Core cleanup failed: $($_.Exception.Message)"
            }
        }
    }
    if ($serverProcess -ne $null) {
        try {
            Stop-Process -Id $serverProcess.Id -Force -ErrorAction SilentlyContinue
            Wait-Process -Id $serverProcess.Id -Timeout 10 -ErrorAction SilentlyContinue
        } catch {
            if ($failure -eq $null) {
                $failure = "Fixture server cleanup failed: $($_.Exception.Message)"
            }
        }
        $listenerAfter = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
        Write-Utf8NoBom -Path (Join-Path $serverDir "shutdown.log") -Text (
            "fixture_server_pid=$($serverProcess.Id)`n" +
            "stopped=$($null -eq $listenerAfter)`n" +
            "port_${Port}_listener=$($null -ne $listenerAfter)`n"
        )
    }
    Restore-EnvironmentValue -Name "ASSISTANT_AGENT_CORE_UI_TOKEN" -Value $previousToken
    Restore-EnvironmentValue -Name "ASSISTANT_UI_PIPE" -Value $previousPipe
    Restore-EnvironmentValue -Name "ASSISTANT_UI_TOKEN_ENV" -Value $previousTokenVariable
}

if ($failure -ne $null) {
    $summary = [pscustomobject]@{
        schema_version = 1
        mode = "desktop-ui-human-approval-local"
        result = "failed"
        failure = $failure
        core_pid = if ($coreProcess -ne $null) { $coreProcess.Id } else { $null }
        ui_pid = if ($uiProcess -ne $null) { $uiProcess.Id } else { $null }
        started_at_utc = $startedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        token_logged = $false
    }
    Write-JsonFile -Path $summaryPath -Value $summary
    Write-Phase -Phase "failed"
    Write-Error $failure
    exit 2
}

exit 0
