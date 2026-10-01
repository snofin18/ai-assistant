Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Parse-Arguments {
    param([string[]]$Arguments)
    $seen = New-Object System.Collections.Generic.HashSet[string]
    $values = @{}
    $selfCheck = $false
    $help = $false

    for ($index = 0; $index -lt $Arguments.Count; $index++) {
        $token = $Arguments[$index]
        if (-not $token.StartsWith("--")) {
            throw "Unexpected argument: $token"
        }
        if (-not $seen.Add($token)) {
            throw "Duplicate option: $token"
        }
        switch ($token) {
            "--help" {
                $help = $true
            }
            "--self-check" {
                $selfCheck = $true
            }
            "--fault" {
                $index++
                if ($index -ge $Arguments.Count -or [string]::IsNullOrWhiteSpace($Arguments[$index]) -or $Arguments[$index].StartsWith("--")) {
                    throw "Option --fault requires a value."
                }
                $values["fault"] = $Arguments[$index]
            }
            "--state-file" {
                $index++
                if ($index -ge $Arguments.Count -or [string]::IsNullOrWhiteSpace($Arguments[$index]) -or $Arguments[$index].StartsWith("--")) {
                    throw "Option --state-file requires a value."
                }
                $values["state-file"] = $Arguments[$index]
            }
            "--document" {
                $index++
                if ($index -ge $Arguments.Count -or [string]::IsNullOrWhiteSpace($Arguments[$index]) -or $Arguments[$index].StartsWith("--")) {
                    throw "Option --document requires a value."
                }
                $values["document"] = $Arguments[$index]
            }
            "--auto-close-ms" {
                $index++
                if ($index -ge $Arguments.Count -or [string]::IsNullOrWhiteSpace($Arguments[$index]) -or $Arguments[$index].StartsWith("--")) {
                    throw "Option --auto-close-ms requires a value."
                }
                $values["auto-close-ms"] = $Arguments[$index]
            }
            default {
                throw "Unknown option: $token"
            }
        }
    }

    return [pscustomobject]@{
        Help = $help
        SelfCheck = $selfCheck
        Fault = if ($values.ContainsKey("fault")) { $values["fault"] } else { "none" }
        StateFile = if ($values.ContainsKey("state-file")) { $values["state-file"] } else { $null }
        Document = if ($values.ContainsKey("document")) { $values["document"] } else { $null }
        AutoCloseMs = if ($values.ContainsKey("auto-close-ms")) { $values["auto-close-ms"] } else { $null }
    }
}

function Show-Usage {
    @"
notepad-like fixture

Usage:
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File notepad-like.ps1 [options]

Options:
  --fault <mode>          none | disappear | timeout | ambiguous | dialog | busy
  --document <path>       Open this file into the editor and save back to it.
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

function Get-SaveAsAutomationIds {
    param([string]$Path)
    $manifest = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    $declared = $manifest.PSObject.Properties["save_as"]
    if ($null -eq $declared) {
        return @()
    }
    return @($manifest.save_as)
}

function Assert-ManifestAutomationIds {
    param(
        [string[]]$RequiredIds,
        [string[]]$RuntimeIds
    )
    if ($RequiredIds.Count -eq 0) {
        throw "automation-ids.json must declare at least one required AutomationId."
    }
    $allIds = @($RequiredIds + $RuntimeIds)
    $duplicateIds = @($allIds | Group-Object | Where-Object { $_.Count -gt 1 })
    if ($duplicateIds.Count -gt 0) {
        throw "Duplicate AutomationId in manifest: $($duplicateIds[0].Name)"
    }
}

function Assert-RequiredAutomationIdProperties {
    param(
        [object]$Window,
        [string[]]$RequiredIds
    )
    foreach ($requiredId in $RequiredIds) {
        $element = if ($requiredId -eq "MainWindow") { $Window } else { $Window.FindName($requiredId) }
        if ($null -eq $element) {
            throw "Required AutomationId element not found: $requiredId"
        }
        $actualId = [System.Windows.Automation.AutomationProperties]::GetAutomationId($element)
        if ($actualId -ne $requiredId) {
            throw "AutomationId mismatch for ${requiredId}: ${actualId}"
        }
    }
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
    $parsedArguments = Parse-Arguments $arguments
    if ($parsedArguments.Help) {
        Show-Usage
        exit 0
    }

    $fault = $parsedArguments.Fault
    Assert-FaultMode $fault

    $stateFile = $parsedArguments.StateFile
    $autoCloseText = $parsedArguments.AutoCloseMs
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
    $saveAsIds = Get-SaveAsAutomationIds $manifestPath
    $allIds = @($requiredIds + $runtimeIds)
    Assert-ManifestAutomationIds -RequiredIds $requiredIds -RuntimeIds @($runtimeIds + $saveAsIds)
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
    Assert-RequiredAutomationIdProperties -Window $window -RequiredIds $requiredIds

    if ($parsedArguments.SelfCheck) {
        $summary = [ordered]@{
            schema_version = "1.0"
            app = "notepad-like"
            status = "ok"
            fault_modes = @("none", "disappear", "timeout", "ambiguous", "dialog", "busy")
            required_automation_ids = $requiredIds
            runtime_automation_ids = $runtimeIds
            save_as_automation_ids = $saveAsIds
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
    $tabCountText = $window.FindName("TabCountText")
    $saveButton = $window.FindName("SaveButton")
    $saveAsButton = $window.FindName("SaveAsButton")
    $addTabButton = $window.FindName("AddTabButton")

    # ---- document binding (T1.2 needs a real file to save back to) ----------
    #
    # Mutable state lives in a hashtable, not in script-scope variables:
    # GetNewClosure() snapshots the caller's variables, so "Dirty" written inside one
    # WPF event handler is invisible to the next one (measured: the title never gained
    # the unsaved marker). A hashtable is a reference, so every closure shares it.
    $docState = @{
        "Path" = $null
        "Tabs" = 1
        "Dirty" = $false
        "Suppress" = $true
    }
    $documentArgument = $parsedArguments.Document
    if (-not [string]::IsNullOrWhiteSpace($documentArgument)) {
        if (-not (Test-Path -LiteralPath $documentArgument -PathType Leaf)) {
            throw "Option --document must point to an existing file: $documentArgument"
        }
        $docState.Path = (Resolve-Path -LiteralPath $documentArgument).Path
        $editor.Text = [System.IO.File]::ReadAllText($docState.Path)
    }
    $applyTitle = {
        $base = if ($null -ne $docState.Path) {
            Split-Path -Leaf $docState.Path
        } else {
            "notepad-like"
        }
        $window.Title = if ($docState.Dirty) { $base + " *" } else { $base }
    }.GetNewClosure()
    & $applyTitle

    $saveButton.Add_Click({
        if ($null -eq $docState.Path) {
            $statusText.Text = "save: no document path (Save As is not implemented yet)"
            return
        }
        [System.IO.File]::WriteAllText($docState.Path, $editor.Text)
        $docState.Dirty = $false
        & $applyTitle
        $statusText.Text = "saved: " + (Split-Path -Leaf $docState.Path)
    }.GetNewClosure())

    $addTabButton.Add_Click({
        $docState.Tabs = $docState.Tabs + 1
        $tabCountText.Text = "Tabs: " + $docState.Tabs
        $docState.Suppress = $true
        $editor.Text = ""
        $docState.Suppress = $false
        $docState.Dirty = $false
        & $applyTitle
        $statusText.Text = "new tab: " + $docState.Tabs
    }.GetNewClosure())

    # ---- Save As: a real cross-process dialog (T1.3) ------------------------
    #
    # The child process owns the dialog window, so the production handler can find
    # it as a separate top-level window and drive it through UIA. The editor text is
    # staged to a temp file first; the dialog copies it to the chosen path. The parent
    # UI thread must stay responsive (a blocked WPF thread would also block UIA reads
    # of the main window), so completion is collected by a polling timer.
    $saveAsState = @{
        "Process" = $null
        "Staging" = $null
        "ResultPath" = $null
        "StartedAt" = $null
        "Timer" = $null
    }
    $saveAsTimer = New-Object System.Windows.Threading.DispatcherTimer
    $saveAsTimer.Interval = [TimeSpan]::FromMilliseconds(200)
    $saveAsState.Timer = $saveAsTimer

    $saveAsTimer.Add_Tick({
        if ($null -eq $saveAsState.Process) {
            return
        }
        $elapsed = ((Get-Date) - $saveAsState.StartedAt).TotalSeconds
        if (-not $saveAsState.Process.HasExited) {
            if ($elapsed -gt 30) {
                $saveAsState.Process.Kill()
                $statusText.Text = "save as: timed out"
                $saveAsTimer.Stop()
                $saveAsState.Process = $null
            }
            return
        }
        $saveAsTimer.Stop()
        $savedTarget = $null
        if ([System.IO.File]::Exists($saveAsState.ResultPath)) {
            $payload = [System.IO.File]::ReadAllText($saveAsState.ResultPath) | ConvertFrom-Json
            if ($payload.saved) {
                $savedTarget = $payload.target
            } elseif ($payload.reason -eq "exists") {
                $statusText.Text = "save as: refused, target already exists"
            } else {
                $statusText.Text = "save as: cancelled"
            }
        } else {
            $statusText.Text = "save as: no result reported"
        }
        if ($null -ne $savedTarget) {
            $docState.Path = $savedTarget
            $docState.Dirty = $false
            & $applyTitle
            $statusText.Text = "saved as: " + (Split-Path -Leaf $savedTarget)
        }
        Remove-Item -LiteralPath $saveAsState.Staging, $saveAsState.ResultPath -ErrorAction SilentlyContinue
        $saveAsState.Process = $null
    }.GetNewClosure())

    $launchSaveAs = {
        if ($null -ne $saveAsState.Process) {
            $statusText.Text = "save as: dialog already open"
            return
        }
        $staging = Join-Path $env:TEMP ("notepad-like-staging-" + $PID + "-" + [guid]::NewGuid().ToString("N") + ".txt")
        [System.IO.File]::WriteAllText($staging, $editor.Text)
        $resultPath = $staging + ".result.json"
        $startDirectory = if ($null -ne $docState.Path) { Split-Path -Parent $docState.Path } else { $env:TEMP }
        $initialName = if ($null -ne $docState.Path) { Split-Path -Leaf $docState.Path } else { "untitled.txt" }
        $dialogScript = Join-Path $PSScriptRoot "save-as-dialog.ps1"
        $saveAsState.Process = Start-Process -FilePath "powershell.exe" -PassThru -ArgumentList @(
            "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $dialogScript,
            "--staging", $staging,
            "--result", $resultPath,
            "--start-directory", $startDirectory,
            "--initial-name", $initialName
        )
        $saveAsState.Staging = $staging
        $saveAsState.ResultPath = $resultPath
        $saveAsState.StartedAt = Get-Date
        $saveAsState.Timer.Start()
        $statusText.Text = "save as: dialog opened"
    }.GetNewClosure()

    $saveAsButton.Add_Click($launchSaveAs)
    $window.Add_KeyDown({
        param($sender, $eventArgs)
        $isCtrlShiftS = $eventArgs.Key -eq [System.Windows.Input.Key]::S -and
            $eventArgs.KeyboardDevice.Modifiers -eq ([System.Windows.Input.ModifierKeys]::Control -bor [System.Windows.Input.ModifierKeys]::Shift)
        if ($isCtrlShiftS) {
            $eventArgs.Handled = $true
            & $launchSaveAs
        }
    }.GetNewClosure())

    $updateCounts = {
        $text = $editor.Text
        $lines = @($text -split "`n")
        $words = @($text -split "\s+" | Where-Object { $_ -ne "" })
        $lineCountText.Text = "Lines: " + $lines.Count
        $wordCountText.Text = "Words: " + $words.Count
        if (-not $docState.Suppress) {
            $docState.Dirty = $true
            & $applyTitle
        }
    }.GetNewClosure()
    $editor.Add_TextChanged($updateCounts)
    & $updateCounts
    $docState.Suppress = $false

    $window.Add_Closed({
        [System.Windows.Threading.Dispatcher]::CurrentDispatcher.InvokeShutdown()
    }.GetNewClosure())
    $window.Show()

    $script:ModalDialog = $null
    if ($autoCloseMs -gt 0) {
        $timer = New-Object System.Windows.Threading.DispatcherTimer
        $timer.Interval = [TimeSpan]::FromMilliseconds($autoCloseMs)
        $timer.Add_Tick({
            $timer.Stop()
            if ($null -ne $script:ModalDialog) {
                $script:ModalDialog.Close()
                $script:ModalDialog = $null
            }
            $window.Close()
        }.GetNewClosure())
        $timer.Start()
    }

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
            $details["behavior"] = "UI thread blocks for 8000 ms after state is written."
            Write-StateFile -Path $stateFile -Fault $fault -Status $status -Details $details -AutomationIds $allIds
            if (-not [string]::IsNullOrWhiteSpace($stateFile)) {
                $probePath = $stateFile + ".probe"
                $probeTimer = New-Object System.Windows.Threading.DispatcherTimer
                $probeTimer.Interval = [TimeSpan]::FromMilliseconds(1000)
                $probeTimer.Add_Tick({
                    $probeTimer.Stop()
                    Set-Content -LiteralPath $probePath -Value "dispatcher-ran" -Encoding ASCII
                }.GetNewClosure())
                $probeTimer.Start()
            }
            $window.Dispatcher.Invoke([System.Action]{ Start-Sleep -Milliseconds 8000 })
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
            $details["behavior"] = "A modal UnexpectedDialog blocks the main window."
            Write-StateFile -Path $stateFile -Fault $fault -Status "fault_pending" -Details $details -AutomationIds $allIds
            $dialog.Add_ContentRendered({
                Write-StateFile -Path $stateFile -Fault $fault -Status "fault_applied" -Details $details -AutomationIds $allIds
            }.GetNewClosure())
            $script:ModalDialog = $dialog
            $dialog.ShowDialog() | Out-Null
            $script:ModalDialog = $null
            $status = "fault_applied"
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

    [System.Windows.Threading.Dispatcher]::Run()
} catch {
    [Console]::Error.WriteLine("notepad-like: " + $_.Exception.Message)
    exit 1
}
