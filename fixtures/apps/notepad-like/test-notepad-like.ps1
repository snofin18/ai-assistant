Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$scriptPath = Join-Path $PSScriptRoot "notepad-like.ps1"
$manifestPath = Join-Path $PSScriptRoot "automation-ids.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$requiredIds = @($manifest.required)
$runtimeIds = @($manifest.runtime)
$allIds = @($requiredIds + $runtimeIds)
$faultModes = @("none", "disappear", "timeout", "ambiguous", "dialog", "busy")
$powershellPath = Join-Path $PSHOME "powershell.exe"
$failures = New-Object System.Collections.Generic.List[string]

function Quote-Argument {
    param([string]$Value)
    return '"' + $Value + '"'
}

function Remove-TestDirectory {
    param([string]$Path)
    $resolved = [IO.Path]::GetFullPath($Path)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove non-temp path: $resolved"
    }
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if (-not (Test-Path -LiteralPath $resolved)) {
            return
        }
        try {
            Remove-Item -LiteralPath $resolved -Recurse -Force -ErrorAction Stop
        } catch {
            Start-Sleep -Milliseconds 100
        }
    }
    if (Test-Path -LiteralPath $resolved) {
        throw "Temp directory was not removed: $resolved"
    }
}

function Stop-TestProcess {
    param([object]$Process)
    if ($null -eq $Process) {
        return
    }
    try {
        if (-not $Process.HasExited) {
            Stop-Process -Id $Process.Id -Force -ErrorAction Stop
        }
        $Process.WaitForExit(5000) | Out-Null
    } catch {
        # The process may have exited between HasExited and Stop-Process.
    }
}

function Wait-StateFile {
    param(
        [string]$Path,
        [string]$ExpectedStatus,
        [object]$Process,
        [int]$TimeoutSeconds = 8
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if ($null -ne $Process -and $Process.HasExited) {
            throw "Process exited before state status ${ExpectedStatus}: exit=$($Process.ExitCode)"
        }
        if (Test-Path -LiteralPath $Path) {
            try {
                $state = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
                if ($state.status -eq $ExpectedStatus) {
                    return $state
                }
            } catch {
                # The app may be replacing the state file; retry.
            }
        }
        Start-Sleep -Milliseconds 100
    }
    throw "State file did not reach status $ExpectedStatus within $TimeoutSeconds seconds: $Path"
}

function Get-WindowByProcessId {
    param([int]$ProcessId)
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
        $ProcessId)
    $window = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
        [System.Windows.Automation.TreeScope]::Children,
        $condition)
    if ($null -eq $window) {
        throw "No top-level UIA window found for pid $ProcessId"
    }
    return $window
}

function Get-ElementsByAutomationId {
    param(
        [System.Windows.Automation.AutomationElement]$Root,
        [string]$AutomationId
    )
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
        $AutomationId)
    $elements = @($Root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition))
    return ,$elements
}

function Assert-FaultSemantics {
    param(
        [string]$Mode,
        [int]$ProcessId,
        [string]$StateFile
    )
    if ($Mode -eq "dialog") {
        $window = Get-WindowByProcessId -ProcessId $ProcessId
        $dialogs = Get-ElementsByAutomationId -Root $window -AutomationId "UnexpectedDialog"
        $visibleDialogs = @($dialogs | Where-Object { -not $_.Current.IsOffscreen })
        if ($visibleDialogs.Count -lt 1) {
            throw "UnexpectedDialog was not rendered"
        }
        return
    }

    if ($Mode -eq "timeout") {
        $probePath = $StateFile + ".probe"
        Start-Sleep -Milliseconds 2000
        if (Test-Path -LiteralPath $probePath) {
            throw "Dispatcher probe ran while timeout fault should block the UI thread"
        }
        return
    }

    $window = Get-WindowByProcessId -ProcessId $ProcessId
    switch ($Mode) {
        "none" {
            $editors = Get-ElementsByAutomationId -Root $window -AutomationId "EditorTextBox"
            if ($editors.Count -ne 1) {
                throw "Expected one visible editor, got $($editors.Count)"
            }
        }
        "disappear" {
            $editors = Get-ElementsByAutomationId -Root $window -AutomationId "EditorTextBox"
            foreach ($editor in $editors) {
                if (-not $editor.Current.IsOffscreen) {
                    throw "EditorTextBox is still on screen after disappear"
                }
            }
        }
        "ambiguous" {
            $editors = Get-ElementsByAutomationId -Root $window -AutomationId "EditorTextBox"
            if ($editors.Count -lt 2) {
                throw "Expected at least two editor matches, got $($editors.Count)"
            }
        }
        "busy" {
            $overlays = Get-ElementsByAutomationId -Root $window -AutomationId "BusyOverlay"
            if ($overlays.Count -ne 1 -or $overlays[0].Current.IsOffscreen) {
                throw "BusyOverlay is not visible"
            }
            $saveButtons = Get-ElementsByAutomationId -Root $window -AutomationId "SaveButton"
            if ($saveButtons.Count -ne 1 -or $saveButtons[0].Current.IsEnabled) {
                throw "SaveButton is not disabled during busy fault"
            }
        }
    }
}

function Assert-InvalidArguments {
    param([string[]]$Arguments)
    $argumentList = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Sta", "-File", (Quote-Argument $scriptPath)) + $Arguments
    $process = Start-Process -FilePath $powershellPath `
        -ArgumentList $argumentList `
        -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -eq 0) {
        throw "Expected non-zero exit for invalid arguments: $($Arguments -join ' ')"
    }
}

$invalidCases = @(
    @("--self-check", "--bogus"),
    @("--self-check", "--fault", "none", "--fault", "busy"),
    @("--self-check", "--fault=busy"),
    @("--self-check", "--state-file", "--self-check"),
    @("--self-check", "--auto-close-ms", "-1"),
    @("--self-check", "--fault"),
    @("--self-check", "positional")
)

foreach ($invalidCase in $invalidCases) {
    try {
        Assert-InvalidArguments -Arguments $invalidCase
        Write-Host "PASS invalid=$($invalidCase -join ' ')"
    } catch {
        $failures.Add("invalid=$($invalidCase -join ' ') : " + $_.Exception.Message) | Out-Null
        Write-Host "FAIL invalid=$($invalidCase -join ' ') : $($_.Exception.Message)"
    }
}

foreach ($mode in $faultModes) {
    $testDirectory = Join-Path ([IO.Path]::GetTempPath()) ("notepad-like-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $testDirectory | Out-Null
    $stateFile = Join-Path $testDirectory "state.json"
    $process = $null

    try {
        $expectedStatus = if ($mode -eq "none") { "ready" } else { "fault_applied" }
        $argumentList = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Sta", "-File", (Quote-Argument $scriptPath),
            "--fault", $mode, "--state-file", (Quote-Argument $stateFile), "--auto-close-ms", "5000")
        $process = Start-Process -FilePath $powershellPath `
            -ArgumentList $argumentList `
            -PassThru -WindowStyle Normal

        $state = Wait-StateFile -Path $stateFile -ExpectedStatus $expectedStatus -Process $process
        if ($state.schema_version -ne "1.0") {
            throw "Unexpected schema_version: $($state.schema_version)"
        }
        if ($state.app -ne "notepad-like") {
            throw "Unexpected app id: $($state.app)"
        }
        if ($state.fault -ne $mode) {
            throw "Expected fault $mode, got $($state.fault)"
        }
        foreach ($requiredId in $allIds) {
            if ($state.automation_ids -notcontains $requiredId) {
                throw "State file missing AutomationId: $requiredId"
            }
        }
        Assert-FaultSemantics -Mode $mode -ProcessId $process.Id -StateFile $stateFile
        Write-Host "PASS fault=$mode"
    } catch {
        $failures.Add("fault=$mode : " + $_.Exception.Message) | Out-Null
        Write-Host "FAIL fault=$mode : $($_.Exception.Message)"
    } finally {
        Stop-TestProcess -Process $process
        try {
            Remove-TestDirectory -Path $testDirectory
        } catch {
            $failures.Add("cleanup=$mode : " + $_.Exception.Message) | Out-Null
            Write-Host "FAIL cleanup=$mode : $($_.Exception.Message)"
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Host "notepad-like tests failed:"
    foreach ($failure in $failures) {
        Write-Host " - $failure"
    }
    exit 1
}

Write-Host "notepad-like tests passed."
exit 0
