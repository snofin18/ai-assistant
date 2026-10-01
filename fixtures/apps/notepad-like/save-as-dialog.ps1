Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Parse-Arguments {
    param([string[]]$Arguments)
    $values = @{}
    for ($index = 0; $index -lt $Arguments.Count; $index++) {
        $token = $Arguments[$index]
        if (-not $token.StartsWith("--")) {
            throw "Unexpected argument: $token"
        }
        $index++
        if ($index -ge $Arguments.Count -or [string]::IsNullOrWhiteSpace($Arguments[$index])) {
            throw "Option $token requires a value."
        }
        switch ($token) {
            "--staging" { $values["staging"] = $Arguments[$index] }
            "--result" { $values["result"] = $Arguments[$index] }
            "--start-directory" { $values["start-directory"] = $Arguments[$index] }
            "--initial-name" { $values["initial-name"] = $Arguments[$index] }
            default { throw "Unknown option: $token" }
        }
    }
    foreach ($required in @("staging", "result", "start-directory", "initial-name")) {
        if (-not $values.ContainsKey($required)) {
            throw "Missing required option: --$required"
        }
    }
    return [pscustomobject]@{
        Staging = $values["staging"]
        Result = $values["result"]
        StartDirectory = $values["start-directory"]
        InitialName = $values["initial-name"]
    }
}

# This dialog runs in its own process on purpose: ADR-0058 / TASK-215 require the
# Save As surface to be cross-process, exactly like the real Win32 #32770 dialog.
try {
    $parsed = Parse-Arguments @($args)
    Add-Type -AssemblyName PresentationFramework

    $xaml = @'
<Window
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
    x:Name="SaveAsDialogWindow"
    Title="Save As"
    Width="560"
    Height="180"
    WindowStartupLocation="CenterScreen"
    AutomationProperties.AutomationId="SaveAsDialogWindow">
  <Grid Margin="16">
    <Grid.RowDefinitions>
      <RowDefinition Height="Auto" />
      <RowDefinition Height="Auto" />
      <RowDefinition Height="*" />
      <RowDefinition Height="Auto" />
    </Grid.RowDefinitions>
    <TextBlock Grid.Row="0" Text="File name" Margin="0,0,0,6" />
    <TextBox
        Grid.Row="1"
        x:Name="SaveAsFileNameBox"
        AutomationProperties.AutomationId="SaveAsFileNameBox" />
    <TextBlock
        Grid.Row="2"
        x:Name="SaveAsStatusText"
        Text="Ready"
        Margin="0,8,0,0"
        TextWrapping="Wrap"
        AutomationProperties.AutomationId="SaveAsStatusText" />
    <StackPanel Grid.Row="3" Orientation="Horizontal" HorizontalAlignment="Right">
      <Button
          x:Name="SaveAsConfirmButton"
          Content="Save"
          Width="96"
          AutomationProperties.AutomationId="SaveAsConfirmButton" />
      <Button
          x:Name="SaveAsCancelButton"
          Content="Cancel"
          Width="96"
          Margin="8,0,0,0"
          AutomationProperties.AutomationId="SaveAsCancelButton" />
    </StackPanel>
  </Grid>
</Window>
'@

    $window = [System.Windows.Markup.XamlReader]::Parse($xaml)
    $fileNameBox = $window.FindName("SaveAsFileNameBox")
    $confirmButton = $window.FindName("SaveAsConfirmButton")
    $cancelButton = $window.FindName("SaveAsCancelButton")
    $statusText = $window.FindName("SaveAsStatusText")

    $fileNameBox.Text = Join-Path $parsed.StartDirectory $parsed.InitialName

    # Hashtable, not a script-scope variable: GetNewClosure() snapshots caller
    # variables, so separate WPF handlers would not see each other's writes.
    $dialogState = @{ "ExitCode" = 1 }

    $writeResult = {
        param([bool]$Saved, [string]$Target, [string]$Reason)
        $payload = [ordered]@{
            saved = $Saved
            target = $Target
            reason = $Reason
        } | ConvertTo-Json -Compress
        [System.IO.File]::WriteAllText($parsed.Result, $payload)
    }.GetNewClosure()

    $confirmButton.Add_Click({
        $target = $fileNameBox.Text
        if ([string]::IsNullOrWhiteSpace($target)) {
            $statusText.Text = "Enter a file name."
            return
        }
        if ([System.IO.File]::Exists($target)) {
            $dialogState.ExitCode = 2
            $statusText.Text = "Refusing to overwrite an existing file."
            & $writeResult $false $target "exists"
            return
        }
        [System.IO.File]::Copy($parsed.Staging, $target, $false)
        $dialogState.ExitCode = 0
        & $writeResult $true $target "saved"
        $window.Close()
    }.GetNewClosure())

    $cancelButton.Add_Click({
        $dialogState.ExitCode = 1
        & $writeResult $false "" "cancelled"
        $window.Close()
    }.GetNewClosure())

    $window.Add_Closed({
        [System.Windows.Threading.Dispatcher]::CurrentDispatcher.InvokeShutdown()
    }.GetNewClosure())

    $window.Show()
    [System.Windows.Threading.Dispatcher]::Run()
    exit $dialogState.ExitCode
} catch {
    [Console]::Error.WriteLine("save-as-dialog: " + $_.Exception.Message)
    exit 1
}
