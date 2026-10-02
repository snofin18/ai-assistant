param(
    [string]$OutputPath = "docs/audits/stage-1a-runtime-validation-2026-10-02-runs.json",
    [int]$RepeatRuns = 10
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../../..")).Path
$logRoot = Join-Path $repoRoot "target/t1-real-uia-logs"
$outputFullPath = Join-Path $repoRoot $OutputPath
$outputDirectory = Split-Path -Parent $outputFullPath
New-Item -ItemType Directory -Force -Path $logRoot | Out-Null
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

$testCases = @(
    @{ Task = "t1.1"; Name = "test_production_t1_1_dry_run_over_real_uia" },
    @{ Task = "t1.2"; Name = "test_production_t1_2_dry_run_over_real_uia" },
    @{ Task = "t1.3"; Name = "test_production_t1_3_dry_run_over_real_uia" }
)

$startedAt = (Get-Date).ToString("o")
$runs = @()

Push-Location $repoRoot
try {
    foreach ($testCase in $testCases) {
        for ($run = 1; $run -le $RepeatRuns; $run++) {
            $logPath = Join-Path $logRoot ($testCase.Task + "-run-" + $run + ".log")
            $start = Get-Date
            $previousErrorPreference = $ErrorActionPreference
            $ErrorActionPreference = "Continue"
            $output = & cargo test -p assistant-agent-core --test production_root_uia $testCase.Name -- --ignored --nocapture 2>&1
            $exitCode = $LASTEXITCODE
            $ErrorActionPreference = $previousErrorPreference
            $finished = Get-Date
            $durationMs = [int]($finished - $start).TotalMilliseconds
            $output | Set-Content -LiteralPath $logPath -Encoding UTF8
            $runs += [ordered]@{
                task = $testCase.Task
                run = $run
                test = $testCase.Name
                exit_code = $exitCode
                passed = ($exitCode -eq 0)
                duration_ms = $durationMs
                log = $logPath
            }
            if ($exitCode -ne 0) {
                Write-Output ("FAILED " + $testCase.Task + " run " + $run + "; log=" + $logPath)
            }
        }
    }
} finally {
    Pop-Location
}

$summary = @()
foreach ($testCase in $testCases) {
    $taskRuns = @($runs | Where-Object { $_.task -eq $testCase.Task })
    $passed = @($taskRuns | Where-Object { $_.passed }).Count
    $failed = $taskRuns.Count - $passed
    $durations = @($taskRuns | ForEach-Object { $_.duration_ms })
    $averageMs = if ($durations.Count -eq 0) { 0 } else { [int](($durations | Measure-Object -Average).Average) }
    $summary += [ordered]@{
        task = $testCase.Task
        runs = $taskRuns.Count
        passed = $passed
        failed = $failed
        average_duration_ms = $averageMs
        minimum_success_rate_met = ($passed -ge [Math]::Ceiling($taskRuns.Count * 0.9))
    }
}

$report = [ordered]@{
    schema_version = 1
    generated_at = (Get-Date).ToString("o")
    started_at = $startedAt
    repeat_runs = $RepeatRuns
    command = "cargo test -p assistant-agent-core --test production_root_uia <test> -- --ignored --nocapture"
    summary = $summary
    runs = $runs
}

$json = $report | ConvertTo-Json -Depth 6
$json = $json -replace "`r`n", "`n"
if (-not $json.EndsWith("`n")) {
    $json += "`n"
}
[System.IO.File]::WriteAllText(
    $outputFullPath,
    $json,
    (New-Object System.Text.UTF8Encoding($false))
)
$report.summary | Format-Table -AutoSize
Write-Output ("report=" + $outputFullPath)
