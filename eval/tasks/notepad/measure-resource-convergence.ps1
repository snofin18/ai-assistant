# Third-round leak audit: measure whether real UIA runs converge to a bounded footprint.
#
# It runs one real UIA task repeatedly and samples the machine after each run. Before each run it
# records the powershell / notepad process counts, its own process handle count and working set,
# and the system available memory. A bounded implementation reaches a steady state: samples from
# the middle onward no longer grow (process counts return to baseline, working set plateaus).

param(
    [string]$Task = "t1.1",
    [int]$RepeatRuns = 30,
    [string]$OutputPath = "docs/audits/leak-audit-round-3-convergence.json"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../../..")).Path
$outputFullPath = Join-Path $repoRoot $OutputPath
$outputDirectory = Split-Path -Parent $outputFullPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

$testNames = @{
    "t1.1" = "test_production_t1_1_dry_run_over_real_uia"
    "t1.2" = "test_production_t1_2_dry_run_over_real_uia"
    "t1.3" = "test_production_t1_3_dry_run_over_real_uia"
}
if (-not $testNames.ContainsKey($Task)) {
    throw "unknown task '$Task' (expected t1.1 / t1.2 / t1.3)"
}
$testName = $testNames[$Task]

function Get-Sample([int]$iteration) {
    $self = Get-Process -Id $PID -ErrorAction SilentlyContinue
    $powershellCount = @(Get-Process -Name powershell -ErrorAction SilentlyContinue).Count
    $notepadCount = @(Get-Process -Name notepad -ErrorAction SilentlyContinue).Count
    $os = Get-CimInstance Win32_OperatingSystem
    return [ordered]@{
        iteration = $iteration
        powershell_processes = $powershellCount
        notepad_processes = $notepadCount
        harness_handles = if ($self) { $self.HandleCount } else { $null }
        harness_working_set_bytes = if ($self) { $self.WorkingSet64 } else { $null }
        available_memory_bytes = [int64]$os.FreePhysicalMemory * 1024
    }
}

$samples = @()
$startedAt = (Get-Date).ToString("o")
Push-Location $repoRoot
try {
    $samples += Get-Sample 0
    for ($run = 1; $run -le $RepeatRuns; $run++) {
        $previous = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        & cargo test -p assistant-agent-core --test production_root_uia $testName -- --ignored --nocapture *> $null
        $exitCode = $LASTEXITCODE
        $ErrorActionPreference = $previous
        if ($exitCode -ne 0) {
            throw "real UIA run $run failed with exit code $exitCode"
        }
        $samples += Get-Sample $run
    }
}
finally {
    Pop-Location
}

$first = $samples[0]
$last = $samples[-1]
$mid = $samples[[int]($samples.Count / 2)]
$report = [ordered]@{
    generated_at = (Get-Date).ToString("o")
    started_at = $startedAt
    task = $Task
    repeat_runs = $RepeatRuns
    samples = $samples
    deltas = [ordered]@{
        powershell_processes_last_minus_first = $last.powershell_processes - $first.powershell_processes
        notepad_processes_last_minus_first = $last.notepad_processes - $first.notepad_processes
        harness_handles_last_minus_first = $last.harness_handles - $first.harness_handles
        harness_working_set_last_minus_first = $last.harness_working_set_bytes - $first.harness_working_set_bytes
        harness_working_set_last_minus_mid = $last.harness_working_set_bytes - $mid.harness_working_set_bytes
    }
}

# Convergence verdict: process counts must return to baseline and the second half must plateau.
$verdict = "converged"
if ($last.powershell_processes -gt $first.powershell_processes) { $verdict = "process_leak" }
elseif ($last.notepad_processes -gt $first.notepad_processes) { $verdict = "process_leak" }
elseif (($last.harness_working_set_bytes - $mid.harness_working_set_bytes) -gt 50MB) { $verdict = "memory_growth" }
$report["verdict"] = $verdict

$json = $report | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText(
    $outputFullPath,
    $json.TrimEnd("`r", "`n") + "`n",
    (New-Object System.Text.UTF8Encoding($false))
)
Write-Output ("verdict={0} powershell={1}->{2} notepad={3}->{4} working_set_mb={5}->{6}" -f `
    $verdict, $first.powershell_processes, $last.powershell_processes, `
    $first.notepad_processes, $last.notepad_processes, `
    [int]($first.harness_working_set_bytes / 1MB), [int]($last.harness_working_set_bytes / 1MB))
