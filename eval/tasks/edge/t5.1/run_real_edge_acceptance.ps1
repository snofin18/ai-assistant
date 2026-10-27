param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("local", "external")]
    [string]$Mode,

    [Parameter(Mandatory = $true)]
    [string]$EvidenceRoot,

    [Parameter(Mandatory = $true)]
    [string]$TaskPackage,

    [Parameter(Mandatory = $true)]
    [string]$CsvBaseUrl,

    [string]$ExpectedCsv = "",
    [string]$ExpectedOrigin = "",
    [int]$Repeat = 10,
    [int]$Port = 8765,
    [string]$EdgeExecutable = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    [string]$ProfileRoot = "$env:LOCALAPPDATA\ai-assistant-task-056-edge-profile",
    [string]$Binary = "target\debug\assistant-agent-core.exe"
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$evidence = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $EvidenceRoot))
$package = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $TaskPackage))
$binaryPath = [System.IO.Path]::GetFullPath((Join-Path $repoRoot $Binary))
$edgePath = [System.IO.Path]::GetFullPath($EdgeExecutable)
$profilePath = [System.IO.Path]::GetFullPath($ProfileRoot)
$fixtureRoot = Join-Path $PSScriptRoot "fixtures"
$localDir = Join-Path $evidence $Mode
$serverDir = Join-Path $evidence "server"

if ($Repeat -lt 1 -or $Repeat -gt 20) {
    throw "Repeat must be in 1..20"
}
foreach ($required in @($package, $binaryPath, $edgePath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required file does not exist: $required"
    }
}
if (-not (Test-Path -LiteralPath $profilePath -PathType Container)) {
    New-Item -ItemType Directory -Path $profilePath | Out-Null
}
New-Item -ItemType Directory -Force -Path $localDir, $serverDir | Out-Null

$firstOutput = Join-Path $localDir "run-01.json"
if (Test-Path -LiteralPath $firstOutput) {
    throw "Evidence already contains $firstOutput; use a new evidence root"
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
$rounds = New-Object System.Collections.Generic.List[object]
$startedAt = (Get-Date).ToUniversalTime().ToString("o")
$edgeVersion = (Get-Item -LiteralPath $edgePath).VersionInfo.ProductVersion

try {
    if ($Mode -eq "local") {
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
        $health = Wait-FixtureServer -Url "http://127.0.0.1:$Port/page-1.html"
        Write-Utf8NoBom -Path (Join-Path $serverDir "health.txt") -Text (
            "fixture_server_pid=$($serverProcess.Id)`n" +
            "status=$($health.StatusCode)`n" +
            "bytes=$($health.RawContentLength)`n"
        )
    }

    for ($run = 1; $run -le $Repeat; $run++) {
        $runLabel = "{0:D2}" -f $run
        $profileId = "task056-$Mode-v3-$runLabel"
        $runProfile = Join-Path $profilePath $profileId
        $jsonPath = Join-Path $localDir "run-$runLabel.json"
        $csvPath = Join-Path $localDir "run-$runLabel.csv"
        $commandLog = Join-Path $localDir "run-$runLabel.command.log"
        $validatorLog = Join-Path $localDir "run-$runLabel.validator.log"
        foreach ($output in @($jsonPath, $csvPath)) {
            if (Test-Path -LiteralPath $output) {
                throw "Output already exists: $output"
            }
        }

        $arguments = @(
            "--browser-task-run",
            "--task-package", $package,
            "--output", $jsonPath,
            "--csv-output", $csvPath,
            "--csv-base-url", $CsvBaseUrl,
            "--approve-step", "navigate_page_1",
            "--approve-step", "navigate_page_2",
            "--approve-step", "navigate_page_3",
            "--browser", "edge",
            "--browser-executable", $edgePath,
            "--browser-profile-root", $profilePath,
            "--browser-profile-id", $profileId,
            "--browser-session-timeout-ms", "60000"
        )
        $commandOutput = & $binaryPath @arguments 2>&1
        $exitCode = $LASTEXITCODE
        Write-Utf8NoBom -Path $commandLog -Text (($commandOutput -join "`n") + "`n")
        if ($exitCode -ne 0) {
            throw "Run $runLabel failed with exit code $exitCode; see $commandLog"
        }
        if (-not (Test-Path -LiteralPath $jsonPath -PathType Leaf)) {
            throw "Run $runLabel did not create JSON output"
        }
        if (-not (Test-Path -LiteralPath $csvPath -PathType Leaf)) {
            throw "Run $runLabel did not create CSV output"
        }

        $result = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
        if ($result.status -ne "completed" -or @($result.steps).Count -ne 6) {
            throw "Run $runLabel JSON status or step count is invalid"
        }
        if (@($result.outputs.PSObject.Properties.Name | Where-Object { $_ -like "page_*_dom" }).Count -ne 3) {
            throw "Run $runLabel JSON does not contain three page DOM outputs"
        }
        if ($result.csv_output.rows -ne 60) {
            throw "Run $runLabel JSON does not report 60 CSV rows"
        }

        $validatorArguments = @(
            (Join-Path $PSScriptRoot "validate_csv.py"),
            "--input", $csvPath,
            "--expected-origin", $CsvBaseUrl
        )
        if ($ExpectedCsv -ne "") {
            $validatorArguments += @("--expected", [System.IO.Path]::GetFullPath((Join-Path $repoRoot $ExpectedCsv)))
        }
        $validatorOutput = & python @validatorArguments 2>&1
        $validatorExit = $LASTEXITCODE
        Write-Utf8NoBom -Path $validatorLog -Text (($validatorOutput -join "`n") + "`n")
        if ($validatorExit -ne 0) {
            throw "Run $runLabel CSV validation failed with exit code $validatorExit"
        }
        Wait-EdgeProfileGone -Profile $runProfile

        $rounds.Add([pscustomobject]@{
            run = $run
            profile_id = $profileId
            json = $jsonPath
            csv = $csvPath
            json_sha256 = (Get-FileHash -LiteralPath $jsonPath -Algorithm SHA256).Hash.ToLowerInvariant()
            csv_sha256 = (Get-FileHash -LiteralPath $csvPath -Algorithm SHA256).Hash.ToLowerInvariant()
            edge_processes_after_run = 0
            validator = ($validatorOutput -join "`n")
        })
        Write-Output "run=$run/$Repeat status=passed"
    }
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
            "port_$($Port)_listener=$($null -ne $listenerAfter)`n"
        )
    }
}

$completedAt = (Get-Date).ToUniversalTime().ToString("o")
$summary = [pscustomobject]@{
    schema_version = 1
    mode = $Mode
    task_package = $package
    csv_base_url = $CsvBaseUrl
    expected_csv = $ExpectedCsv
    edge_executable = $edgePath
    edge_version = $edgeVersion
    profile_root = $profilePath
    started_at_utc = $startedAt
    completed_at_utc = $completedAt
    required_rounds = $Repeat
    successful_rounds = $rounds.Count
    success_rate = if ($Repeat -eq 0) { 0 } else { $rounds.Count / $Repeat }
    result = if ($failure -eq $null) { "passed" } else { "failed" }
    failure = $failure
    rounds = $rounds
}
Write-JsonFile -Path (Join-Path $evidence "summary.json") -Value $summary

if ($failure -ne $null) {
    Write-Error $failure
    exit 2
}
exit 0
