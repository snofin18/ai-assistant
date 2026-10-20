param(
    [Parameter(Mandatory = $true)]
    [string]$TitleContains
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$windowCondition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Window
)
$windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
    [System.Windows.Automation.TreeScope]::Children,
    $windowCondition
)
$target = $null
foreach ($window in $windows) {
    if ($window.Current.Name -like "*$TitleContains*") {
        $target = $window
        break
    }
}
if ($null -eq $target) {
    [pscustomobject]@{
        found_window = $false
        title = $null
        address_bar_names = @()
        tab_names = @()
        sampled_control_count = 0
    } | ConvertTo-Json -Depth 4
    exit 0
}

$descendants = $target.FindAll(
    [System.Windows.Automation.TreeScope]::Descendants,
    [System.Windows.Automation.Condition]::TrueCondition
)
$sample = @()
foreach ($element in $descendants) {
    if ($sample.Count -ge 400) {
        break
    }
    $sample += [pscustomobject]@{
        name = $element.Current.Name
        automation_id = $element.Current.AutomationId
        control_type = $element.Current.ControlType.ProgrammaticName
    }
}
$addressNames = @(
    $sample |
        Where-Object {
            $_.control_type -match "Edit|ComboBox|Document" -and
            $_.name -match "address|search|url"
        } |
        Select-Object -ExpandProperty name
)
$tabNames = @(
    $sample |
        Where-Object { $_.control_type -match "TabItem" } |
        Select-Object -ExpandProperty name
)
[pscustomobject]@{
    found_window = $true
    title = $target.Current.Name
    address_bar_names = $addressNames
    tab_names = $tabNames
    sampled_control_count = $sample.Count
} | ConvertTo-Json -Depth 4
