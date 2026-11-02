param(
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [string]$EdgeExecutable = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    [string]$ProfileRoot = "$env:LOCALAPPDATA\ai-assistant-task-356-edge-profile",
    [string]$Binary = "target\debug\assistant-agent-core.exe",
    [int]$Port = 8766
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
$binaryPath = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $Binary))
$edgePath = [System.IO.Path]::GetFullPath($EdgeExecutable)
$profilePath = [System.IO.Path]::GetFullPath($ProfileRoot)
$packagePath = Join-Path $repoRoot "adapters\browser.edge\tasks\t5.2.profile-human-approval.json"
$fixtureRoot = Join-Path $PSScriptRoot "fixtures"
$serverDir = Join-Path $evidence "server"
$runDir = Join-Path $evidence "human"

if ($Port -lt 1 -or $Port -gt 65535) {
    throw "Port must be in 1..65535"
}
foreach ($required in @($binaryPath, $edgePath, $packagePath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required file does not exist: $required"
    }
}
if (-not (Test-Path -LiteralPath $profilePath -PathType Container)) {
    New-Item -ItemType Directory -Path $profilePath | Out-Null
}
New-Item -ItemType Directory -Force -Path $serverDir, $runDir | Out-Null

$jsonPath = Join-Path $runDir "result.json"
$commandLog = Join-Path $runDir "run.command.log"
if (Test-Path -LiteralPath $jsonPath) {
    throw "Result already exists: $jsonPath"
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

$serverProcess = $null
$failure = $null
$startedAt = (Get-Date).ToUniversalTime().ToString("o")
$edgeVersion = (Get-Item -LiteralPath $edgePath).VersionInfo.ProductVersion

try {
    $listener = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    if ($listener) {
        throw "Port $Port already has a listener"
    }
    $python = (Get-Command python).Source
    $generateOutput = & $python (Join-Path $PSScriptRoot "generate_fixtures.py") 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Fixture generation failed: $generateOutput"
    }
    Write-Utf8NoBom -Path (Join-Path $evidence "fixture-manifest.stdout.log") -Text (($generateOutput -join "`n") + "`n")

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

    $profileId = "task356-human-01"
    $runProfile = Join-Path $profilePath $profileId
    $arguments = @(
        "--browser-task-run",
        "--task-package", $packagePath,
        "--output", $jsonPath,
        "--human-approval",
        "--approve-step", "navigate_form",
        "--approve-step", "fill_form",
        "--browser", "edge",
        "--browser-executable", $edgePath,
        "--browser-profile-root", $profilePath,
        "--browser-profile-id", $profileId,
        "--browser-session-timeout-ms", "180000"
    )
    $commandOutput = & $binaryPath @arguments 2>&1 | Tee-Object -FilePath $commandLog
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        throw "Human approval run failed with exit code $exitCode; see $commandLog"
    }
    if (-not (Test-Path -LiteralPath $jsonPath -PathType Leaf)) {
        throw "Human approval run did not create result JSON"
    }
    $result = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
    if ($result.status -ne "completed" -or @($result.steps).Count -ne 4) {
        throw "Result status or step count is invalid"
    }
    if ($result.outputs.submitted -ne $true) {
        throw "Result does not report submitted=true"
    }
    $expectedPostUrl = "http://127.0.0.1:$Port/t5.2/submitted"
    $postUrl = [string]$result.outputs.post_url
    if ($postUrl -ne $expectedPostUrl) {
        throw "Result post_url is not the query-free local success page"
    }
    $success = Invoke-WebRequest -UseBasicParsing -Uri $postUrl -TimeoutSec 5
    if ($success.StatusCode -ne 200 -or $success.Content -notmatch "submitted-marker") {
        throw "Success page marker was not observed"
    }
    Wait-EdgeProfileGone -Profile $runProfile

    $summary = [pscustomobject]@{
        schema_version = 1
        mode = "human-approval-local"
        result = "passed"
        task_package = $packagePath
        edge_executable = $edgePath
        edge_version = $edgeVersion
        profile_root = $profilePath
        profile_id = $profileId
        output_json = $jsonPath
        command_log = $commandLog
        started_at_utc = $startedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        edge_processes_after_run = 0
        post_url = $result.outputs.post_url
        submitted_marker = "submitted-marker"
    }
    Write-JsonFile -Path (Join-Path $evidence "summary.json") -Value $summary
} catch {
    $failure = $_.Exception.Message
} finally {
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
}

if ($failure -ne $null) {
    $summary = [pscustomobject]@{
        schema_version = 1
        mode = "human-approval-local"
        result = "failed"
        failure = $failure
        started_at_utc = $startedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    }
    Write-JsonFile -Path (Join-Path $evidence "summary.json") -Value $summary
    Write-Error $failure
    exit 2
}

exit 0
