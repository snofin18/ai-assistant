Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-OptionValue {
    param(
        [string[]]$Arguments,
        [string]$Name
    )
    for ($index = 0; $index -lt $Arguments.Count; $index++) {
        if ($Arguments[$index] -eq $Name) {
            if (($index + 1) -ge $Arguments.Count) {
                throw "Option $Name requires a value."
            }
            return $Arguments[$index + 1]
        }
    }
    return $null
}

function Test-Option {
    param(
        [string[]]$Arguments,
        [string]$Name
    )
    return $Arguments -contains $Name
}

function Show-Usage {
    @"
notepad-like fixture

Usage:
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File notepad-like.ps1 [options]

Options:
  --fault <mode>          none | disappear | timeout | ambiguous | dialog | busy
  --state-file <path>     Write startup state JSON to this path.
  --auto-close-ms <n>     Close the window after n milliseconds when possible.
  --self-check            Validate XAML and AutomationId manifest, then exit.
  --help                  Show this help.
"@
}

function Get-RequiredAutomationIds {
    param([string]$Path)
    $manifest = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    return @($manifest.required)
}

function Get-RuntimeAutomationIds {
    param([string]$Path)
    $manifest = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    return @($manifest.runtime)
}

function Assert-FaultMode {
    param([string]$Mode)
    $allowed = @("none", "disappear", "timeout", "ambiguous", "dialog", "busy")
    if ($allowed -notcontains $Mode) {
        throw "Unknown fault mode: $Mode"
    }
}

function Write-StateFile {
    param(
        [string]$Path,
        [string]$Fault,
        [string]$Status,
        [object]$Details,
        [string[]]$AutomationIds
    )
    if ([string]::IsNullOrWhiteSpace($Path)) {
        return
    }
    $state = [ordered]@{
        schema_version = "1.0"
        app = "notepad-like"
        pid = $PID
        fault = $Fault
        status = $Status
        started_at = $script:StartedAt
        window_title = "notepad-like"
        automation_ids = $AutomationIds
        details = $Details
    }
    $state | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Path -Encoding UTF8
}

function Get-XamlAutomationIds {
    param([string]$Xaml)
    $matches = [regex]::Matches($Xaml, 'AutomationProperties\.AutomationId="([^"]+)"')
    return @($matches | ForEach-Object { $_.Groups[1].Value })
}

try {
    $arguments = @($args)
    if (Test-Option $arguments "--help") {
        Show-Usage
        exit 0
    }

    $fault = Get-OptionValue $arguments "--fault"
    if ($null -eq $fault) {
        $fault = "none"
    }
    Assert-FaultMode $fault

    $stateFile = Get-OptionValue $arguments "--state-file"
    $autoCloseText = Get-OptionValue $arguments "--auto-close-ms"
    $autoCloseMs = 0
    if ($null -ne $autoCloseText) {
        if (-not [int]::TryParse($autoCloseText, [ref]$autoCloseMs) -or $autoCloseMs -lt 0) {
            throw "Option --auto-close-ms must be a non-negative integer."
        }
    }

    Add-Type -AssemblyName PresentationFramework
    $xamlPath = Join-Path $PSScriptRoot "MainWindow.xaml"
    $manifestPath = Join-Path $PSScriptRoot "automation-ids.json"
    $xaml = Get-Content -LiteralPath $xamlPath -Raw
    $requiredIds = Get-RequiredAutomationIds $manifestPath
    $runtimeIds = Get-RuntimeAutomationIds $manifestPath
    $allIds = @($requiredIds + $runtimeIds)
    $xamlIds = Get-XamlAutomationIds $xaml

    foreach ($requiredId in $requiredIds) {
        if ($xamlIds -notcontains $requiredId) {
            throw "Missing required AutomationId: $requiredId"
        }
    }
    $duplicateIds = @($xamlIds | Group-Object | Where-Object { $_.Count -gt 1 })
    if ($duplicateIds.Count -gt 0) {
        throw "Duplicate AutomationId in base XAML: $($duplicateIds[0].Name)"
    }

    $window = [System.Windows.Markup.XamlReader]::Parse($xaml)

    if (Test-Option $arguments "--self-check") {
        $summary = [ordered]@{
            schema_version = "1.0"
            app = "notepad-like"
            status = "ok"
            fault_modes = @("none", "disappear", "timeout", "ambiguous", "dialog", "busy")
            required_automation_ids = $requiredIds
            runtime_automation_ids = $runtimeIds
        }
        $summary | ConvertTo-Json -Depth 5 -Compress
        exit 0
    }

    $script:StartedAt = (Get-Date).ToString("o")
    $editor = $window.FindName("EditorTextBox")
    $editorHost = $window.FindName("EditorHost")
    $busyOverlay = $window.FindName("BusyOverlay")
    $statusText = $window.FindName("StatusText")
    $faultStatusText = $window.FindName("FaultStatusText")
    $lineCountText = $window.FindName("LineCountText")
    $wordCountText = $window.FindName("WordCountText")

    $updateCounts = {
        $text = $editor.Text
        $lines = @($text -split "`n")
        $words = @($text -split "\s+" | Where-Object { $_ -ne "" })
        $lineCountText.Text = "Lines: " + $lines.Count
        $wordCountText.Text = "Words: " + $words.Count
    }.GetNewClosure()
    $editor.Add_TextChanged($updateCounts)
    & $updateCounts

    $window.Add_Closed({
        [System.Windows.Threading.Dispatcher]::CurrentDispatcher.InvokeShutdown()
    }.GetNewClosure())
    $window.Show()

    $details = [ordered]@{}
    $status = "ready"
    switch ($fault) {
        "disappear" {
            $editor.Visibility = [System.Windows.Visibility]::Collapsed
            $faultStatusText.Text = "Fault: disappear"
            $status = "fault_applied"
            $details["behavior"] = "EditorTextBox is collapsed after startup."
        }
        "timeout" {
            $faultStatusText.Text = "Fault: timeout"
            $status = "fault_applied"
            $details["behavior"] = "UI thread blocks for 5000 ms after state is written."
            Write-StateFile -Path $stateFile -Fault $fault -Status $status -Details $details -AutomationIds $allIds
            Start-Sleep -Milliseconds 5000
        }
        "ambiguous" {
            $duplicate = New-Object System.Windows.Controls.TextBox
            $duplicate.Name = "EditorTextBoxDuplicate"
            $duplicate.Text = "duplicate editor for ambiguous selector testing"
            $duplicate.Height = 60
            [System.Windows.Automation.AutomationProperties]::SetAutomationId($duplicate, "EditorTextBox")
            $editorHost.Children.Add($duplicate) | Out-Null
            $faultStatusText.Text = "Fault: ambiguous"
            $status = "fault_applied"
            $details["behavior"] = "Two visible TextBox elements share AutomationId EditorTextBox."
        }
        "dialog" {
            $dialog = New-Object System.Windows.Window
            $dialog.Title = "Unexpected Dialog"
            $dialog.Owner = $window
            $dialog.WindowStartupLocation = [System.Windows.WindowStartupLocation]::CenterOwner
            $dialog.SizeToContent = [System.Windows.SizeToContent]::WidthAndHeight
            [System.Windows.Automation.AutomationProperties]::SetAutomationId($dialog, "UnexpectedDialog")

            $panel = New-Object System.Windows.Controls.StackPanel
            $panel.Margin = [System.Windows.Thickness]::new(16)
            $message = New-Object System.Windows.Controls.TextBlock
            $message.Text = "Unexpected dialog injected by notepad-like."
            [System.Windows.Automation.AutomationProperties]::SetAutomationId($message, "DialogMessageText")
            $panel.Children.Add($message) | Out-Null

            $buttons = New-Object System.Windows.Controls.StackPanel
            $buttons.Orientation = [System.Windows.Controls.Orientation]::Horizontal
            $buttons.Margin = [System.Windows.Thickness]::new(0, 12, 0, 0)
            $cancel = New-Object System.Windows.Controls.Button
            $cancel.Content = "Cancel"
            $cancel.Width = 96
            [System.Windows.Automation.AutomationProperties]::SetAutomationId($cancel, "DialogCancelButton")
            $cancel.Add_Click({ $dialog.Close() }.GetNewClosure())
            $continue = New-Object System.Windows.Controls.Button
            $continue.Content = "Continue"
            $continue.Width = 96
            $continue.Margin = [System.Windows.Thickness]::new(8, 0, 0, 0)
            [System.Windows.Automation.AutomationProperties]::SetAutomationId($continue, "DialogContinueButton")
            $continue.Add_Click({ $dialog.Close() }.GetNewClosure())
            $buttons.Children.Add($cancel) | Out-Null
            $buttons.Children.Add($continue) | Out-Null
            $panel.Children.Add($buttons) | Out-Null
            $dialog.Content = $panel

            $faultStatusText.Text = "Fault: dialog"
            $status = "fault_applied"
            $details["behavior"] = "A modal UnexpectedDialog blocks the main window."
            Write-StateFile -Path $stateFile -Fault $fault -Status $status -Details $details -AutomationIds $allIds
            $dialog.ShowDialog() | Out-Null
        }
        "busy" {
            $busyOverlay.Visibility = [System.Windows.Visibility]::Visible
            $window.FindName("OpenButton").IsEnabled = $false
            $window.FindName("SaveButton").IsEnabled = $false
            $window.FindName("SaveAsButton").IsEnabled = $false
            $editor.IsEnabled = $false
            $faultStatusText.Text = "Fault: busy"
            $status = "fault_applied"
            $details["behavior"] = "BusyOverlay is visible and write controls are disabled."
        }
    }

    Write-StateFile -Path $stateFile -Fault $fault -Status $status -Details $details -AutomationIds $allIds

    if ($autoCloseMs -gt 0) {
        $timer = New-Object System.Windows.Threading.DispatcherTimer
        $timer.Interval = [TimeSpan]::FromMilliseconds($autoCloseMs)
        $timer.Add_Tick({
            $timer.Stop()
            $window.Close()
        }.GetNewClosure())
        $timer.Start()
    }

    [System.Windows.Threading.Dispatcher]::Run()
} catch {
    [Console]::Error.WriteLine("notepad-like: " + $_.Exception.Message)
    exit 1
}
