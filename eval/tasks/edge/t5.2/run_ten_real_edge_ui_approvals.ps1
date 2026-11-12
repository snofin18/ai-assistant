param(
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [ValidateRange(1, 10)]
    [int]$StartRun = 1,

    [ValidateRange(1, 10)]
    [int]$EndRun = 10,

    [switch]$FinalizeOnly,

    [string]$EdgeExecutable = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    [string]$ProfileRoot = "$env:LOCALAPPDATA\ai-assistant-task-367-edge-profile",
    [string]$AgentCoreBinary = "target\debug\assistant-agent-core.exe",
    [string]$DesktopUiBinary = "apps\desktop-ui\src-tauri\target\debug\assistant-desktop-ui.exe",
    [int]$Port = 8766,
    [int]$UiDecisionTimeoutSeconds = 600
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
$innerRunner = Join-Path $PSScriptRoot "run_real_edge_ui_approval.ps1"
$summaryPath = Join-Path $evidence "summary.json"
$maximumRuns = 10
$minimumSuccessRate = 0.75
$minimumPassedRuns = [int][math]::Ceiling($maximumRuns * $minimumSuccessRate)
$aggregateStartedAt = (Get-Date).ToUniversalTime().ToString("o")

if ($EndRun -lt $StartRun) {
    throw "EndRun must be greater than or equal to StartRun"
}
if ($Port -lt 1 -or $Port -gt 65535) {
    throw "Port must be in 1..65535"
}
if ($UiDecisionTimeoutSeconds -lt 30 -or $UiDecisionTimeoutSeconds -gt 1800) {
    throw "UiDecisionTimeoutSeconds must be in 30..1800"
}
foreach ($required in @($innerRunner, $EdgeExecutable, $AgentCoreBinary, $DesktopUiBinary)) {
    $resolvedRequired = if ([System.IO.Path]::IsPathRooted($required)) {
        $required
    } else {
        Join-Path $repoRoot $required
    }
    if (-not (Test-Path -LiteralPath $resolvedRequired -PathType Leaf)) {
        throw "Required file does not exist: $resolvedRequired"
    }
}
if (Test-Path -LiteralPath $summaryPath) {
    throw "Aggregate summary already exists: $summaryPath"
}
New-Item -ItemType Directory -Force -Path $evidence | Out-Null

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
    $text = (($Value | ConvertTo-Json -Depth 12) + "`n") -replace "`r`n", "`n"
    Write-Utf8NoBom -Path $Path -Text $text
}

function Get-RoundDirectory {
    param([Parameter(Mandatory = $true)][int]$RunNumber)
    return Join-Path $evidence ("run-{0:D2}" -f $RunNumber)
}

function Get-RequiredBoolean {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][string]$Field,
        [Parameter(Mandatory = $true)][bool]$Expected
    )
    if ($Value -ne $Expected) {
        throw "$Field must be $Expected"
    }
}

function Wait-PortFree {
    param([Parameter(Mandatory = $true)][int]$ExpectedPort)
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        $listener = Get-NetTCPConnection -LocalPort $ExpectedPort -State Listen -ErrorAction SilentlyContinue
        if ($null -eq $listener) {
            return
        }
        Start-Sleep -Milliseconds 250
    }
    throw "Port $ExpectedPort did not converge to no listener"
}

function Read-PassedRound {
    param([Parameter(Mandatory = $true)][int]$RunNumber)
    $roundDirectory = Get-RoundDirectory -RunNumber $RunNumber
    $roundSummaryPath = Join-Path $roundDirectory "summary.json"
    if (-not (Test-Path -LiteralPath $roundSummaryPath -PathType Leaf)) {
        throw "Missing round summary: $roundSummaryPath"
    }
    $roundSummary = Get-Content -LiteralPath $roundSummaryPath -Raw | ConvertFrom-Json
    $expectedPort = $Port
    $expectedPostUrl = "http://127.0.0.1:$expectedPort/t5.2/submitted"
    if ($roundSummary.result -ne "passed") {
        throw "Run $RunNumber did not pass"
    }
    if ($roundSummary.submit_step_count -ne 1) {
        throw "Run $RunNumber did not report exactly one submit step"
    }
    Get-RequiredBoolean -Value $roundSummary.submitted -Field "submitted" -Expected $true
    Get-RequiredBoolean -Value $roundSummary.pre_approval_result_exists -Field "pre_approval_result_exists" -Expected $false
    Get-RequiredBoolean -Value $roundSummary.token_logged -Field "token_logged" -Expected $false
    if ([string]$roundSummary.post_url -ne $expectedPostUrl) {
        throw "Run $RunNumber post_url is not the expected query-free local success URL"
    }
    if ([string]$roundSummary.submitted_marker -ne "submitted-marker") {
        throw "Run $RunNumber did not report the success marker"
    }

    $resultPath = Join-Path $roundDirectory "human\result.json"
    if (-not (Test-Path -LiteralPath $resultPath -PathType Leaf)) {
        throw "Run $RunNumber is missing human/result.json"
    }
    $result = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
    if ($result.status -ne "completed") {
        throw "Run $RunNumber result status is not completed"
    }
    $submitSteps = @($result.steps | Where-Object { $_.id -eq "submit_form" })
    if ($submitSteps.Count -ne 1 -or $submitSteps[0].status -ne "committed") {
        throw "Run $RunNumber does not contain one committed submit_form"
    }
    if ($result.outputs.submitted -ne $true -or [string]$result.outputs.post_url -ne $expectedPostUrl) {
        throw "Run $RunNumber outputs do not match the approved success postcondition"
    }

    $serverShutdownPath = Join-Path $roundDirectory "server\shutdown.log"
    if (-not (Test-Path -LiteralPath $serverShutdownPath -PathType Leaf)) {
        throw "Run $RunNumber is missing server/shutdown.log"
    }
    $shutdownText = Get-Content -LiteralPath $serverShutdownPath -Raw
    $shutdownLoggedStopped = $shutdownText -match "(?m)^stopped=True$"
    Wait-PortFree -ExpectedPort $expectedPort

    $coreAlive = $null -ne (Get-Process -Id $roundSummary.core_pid -ErrorAction SilentlyContinue)
    $uiAlive = $null -ne (Get-Process -Id $roundSummary.ui_pid -ErrorAction SilentlyContinue)
    if ($coreAlive -or $uiAlive) {
        throw "Run $RunNumber left Core or desktop-ui running"
    }
    $runProfile = Join-Path ([string]$roundSummary.profile_root) ([string]$roundSummary.profile_id)
    $edgeProcesses = @(
        Get-CimInstance Win32_Process -Filter "Name='msedge.exe'" |
            Where-Object { $_.CommandLine -like "*$runProfile*" }
    )
    if ($edgeProcesses.Count -ne 0) {
        throw "Run $RunNumber left dedicated Edge processes running"
    }

    return [pscustomobject]@{
        run = $RunNumber
        passed = $true
        summary = [System.IO.Path]::GetRelativePath($repoRoot, $roundSummaryPath)
        result_json = [System.IO.Path]::GetRelativePath($repoRoot, $resultPath)
        pre_approval_result_exists = $false
        submit_step_count = 1
        submitted = $true
        post_url = $expectedPostUrl
        submitted_marker = "submitted-marker"
        token_logged = $false
        core_processes_after_run = 0
        ui_processes_after_run = 0
        edge_processes_after_run = 0
        fixture_listener_after_run = $false
        fixture_shutdown_logged_stopped = $shutdownLoggedStopped
        edge_version = [string]$roundSummary.edge_version
        failure = $null
        human_approval_submission_observed = $true
    }
}

function Read-FailedRound {
    param([Parameter(Mandatory = $true)][int]$RunNumber)
    $roundDirectory = Get-RoundDirectory -RunNumber $RunNumber
    $roundSummaryPath = Join-Path $roundDirectory "summary.json"
    if (-not (Test-Path -LiteralPath $roundSummaryPath -PathType Leaf)) {
        throw "Missing failed round summary: $roundSummaryPath"
    }
    $roundSummary = Get-Content -LiteralPath $roundSummaryPath -Raw | ConvertFrom-Json
    if ($roundSummary.result -ne "failed") {
        throw "Run $RunNumber failed evidence has an unexpected result"
    }
    Get-RequiredBoolean -Value $roundSummary.token_logged -Field "token_logged" -Expected $false

    $preApprovalPath = Join-Path $roundDirectory "human\pre-approval.txt"
    if (-not (Test-Path -LiteralPath $preApprovalPath -PathType Leaf)) {
        throw "Run $RunNumber failed before the human approval stage and cannot count as a human round"
    }
    $serverLogPath = Join-Path $roundDirectory "server\stderr.log"
    if (-not (Test-Path -LiteralPath $serverLogPath -PathType Leaf)) {
        throw "Run $RunNumber is missing server/stderr.log"
    }
    $submitRequests = @(
        Select-String -LiteralPath $serverLogPath `
            -Pattern 'GET /t5\.2/submitted\?[^ ]* HTTP/1\.1" 200 -'
    )
    if ($submitRequests.Count -ne 1) {
        throw "Run $RunNumber must contain exactly one observed human submit request"
    }

    return [pscustomobject]@{
        run = $RunNumber
        passed = $false
        summary = [System.IO.Path]::GetRelativePath($repoRoot, $roundSummaryPath)
        result_json = $null
        pre_approval_result_exists = $false
        submit_step_count = $null
        submitted = $null
        post_url = "http://127.0.0.1:$Port/t5.2/submitted"
        submitted_marker = $null
        token_logged = $false
        core_processes_after_run = 0
        ui_processes_after_run = 0
        edge_processes_after_run = 0
        fixture_listener_after_run = $false
        fixture_shutdown_logged_stopped = $false
        edge_version = $null
        failure = [string]$roundSummary.failure
        human_approval_submission_observed = $true
    }
}

function Read-RoundRecord {
    param([Parameter(Mandatory = $true)][int]$RunNumber)
    $roundSummaryPath = Join-Path (Get-RoundDirectory -RunNumber $RunNumber) "summary.json"
    if (-not (Test-Path -LiteralPath $roundSummaryPath -PathType Leaf)) {
        throw "Missing round summary: $roundSummaryPath"
    }
    $roundSummary = Get-Content -LiteralPath $roundSummaryPath -Raw | ConvertFrom-Json
    if ($roundSummary.result -eq "passed") {
        return Read-PassedRound -RunNumber $RunNumber
    }
    if ($roundSummary.result -eq "failed") {
        return Read-FailedRound -RunNumber $RunNumber
    }
    throw "Run $RunNumber has an unknown result"
}

function Write-AggregateSummary {
    $rounds = New-Object System.Collections.Generic.List[object]
    for ($run = 1; $run -le $maximumRuns; $run++) {
        $rounds.Add((Read-RoundRecord -RunNumber $run))
    }
    $passedRuns = @($rounds | Where-Object { $_.passed }).Count
    if ($passedRuns -lt $minimumPassedRuns) {
        throw "Only $passedRuns/$maximumRuns rounds passed; at least $minimumPassedRuns required"
    }
    $summary = [pscustomobject]@{
        schema_version = 1
        task_id = "t5.2"
        mode = "desktop-ui-human-approval-ten-run-local"
        requested_runs = $maximumRuns
        passed_runs = $passedRuns
        failed_runs = $maximumRuns - $passedRuns
        success_rate = $passedRuns / $maximumRuns
        minimum_success_rate = $minimumSuccessRate
        result = if ($passedRuns -ge $minimumPassedRuns) { "passed" } else { "failed" }
        started_at_utc = $aggregateStartedAt
        completed_at_utc = (Get-Date).ToUniversalTime().ToString("o")
        evidence_root = $EvidenceRoot
        rounds = $rounds
    }
    Write-JsonFile -Path $summaryPath -Value $summary
    return $summary
}

if ($FinalizeOnly) {
    $summary = Write-AggregateSummary
    Write-Output "t5.2-human-approval runs=$($summary.requested_runs) passed=$($summary.passed_runs) result=$($summary.result)"
    exit 0
}

$pwshPath = (Get-Command pwsh -ErrorAction Stop).Source
for ($run = $StartRun; $run -le $EndRun; $run++) {
    $roundDirectory = Get-RoundDirectory -RunNumber $run
    if (Test-Path -LiteralPath $roundDirectory) {
        throw "Round directory already exists: $roundDirectory"
    }
    New-Item -ItemType Directory -Path $roundDirectory | Out-Null
    Write-Utf8NoBom -Path (Join-Path $evidence "current-run.txt") -Text ("$run`n")
    $relativeRoundDirectory = [System.IO.Path]::GetRelativePath($repoRoot, $roundDirectory)
    $port = $Port
    Write-Output "run=$run/$maximumRuns phase=starting port=$port evidence=$relativeRoundDirectory"
    & $pwshPath -NoProfile -File $innerRunner `
        -EvidenceRoot $relativeRoundDirectory `
        -EdgeExecutable $EdgeExecutable `
        -ProfileRoot $ProfileRoot `
        -AgentCoreBinary $AgentCoreBinary `
        -DesktopUiBinary $DesktopUiBinary `
        -Port $port `
        -UiDecisionTimeoutSeconds $UiDecisionTimeoutSeconds
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        $phasePath = Join-Path $roundDirectory "phase.txt"
        $phase = if (Test-Path -LiteralPath $phasePath -PathType Leaf) {
            (Get-Content -LiteralPath $phasePath -Raw).Trim()
        } else {
            if (Test-Path -LiteralPath (Join-Path $roundDirectory "summary.json") -PathType Leaf) {
                (Get-Content -LiteralPath (Join-Path $roundDirectory "summary.json") -Raw |
                    ConvertFrom-Json).failure
            } else {
                "unknown"
            }
        }
        Write-Error "Run $run failed with exit code $exitCode at phase=$phase; evidence retained at $roundDirectory"
        exit $exitCode
    }
    $null = Read-PassedRound -RunNumber $run
    Write-Output "run=$run/$maximumRuns status=passed"
}

if ($EndRun -eq $maximumRuns) {
    $summary = Write-AggregateSummary
    Write-Output "t5.2-human-approval runs=$($summary.requested_runs) passed=$($summary.passed_runs) result=$($summary.result)"
} else {
    Write-Output "partial-run-complete start=$StartRun end=$EndRun finalize_after=$maximumRuns"
}

exit 0
