Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$WorkDir = 'D:\csart\eol-probe'
if (-not (Test-Path $WorkDir)) { New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null }

$nonce = ([Guid]::NewGuid().ToString('N')).Substring(0, 8)
$path  = Join-Path $WorkDir ("probe05dbg-{0}.txt" -f $nonce)
[System.IO.File]::WriteAllText($path, ('a' * 256))

Write-Host "[1] created file: $path"

$null = Start-Process -FilePath 'notepad.exe' -ArgumentList "`"$path`"" -PassThru -WindowStyle Hidden
Write-Host "[2] started notepad, waiting 3s..."
Start-Sleep -Seconds 3

$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$CT = [System.Windows.Automation.ControlType]
$root = $AE::RootElement

# enumerate all Notepad windows
$winCond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Notepad')
$wins = $root.FindAll($TS::Children, $winCond)
Write-Host "[3] found $($wins.Count) Notepad windows"
foreach ($w in $wins) {
  Write-Host "  window name='$($w.Current.Name)'"
  $tabCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::TabItem)
  $tabs = $w.FindAll($TS::Descendants, $tabCond)
  Write-Host "  found $($tabs.Count) TabItems:"
  foreach ($t in $tabs) {
    Write-Host "    TabItem name='$($t.Current.Name)'"
  }
}

# now try to find via nonce
$expectedName = "probe05dbg-$nonce.txt"
Write-Host "[4] searching for nonce='$expectedName'"
$found = $false
foreach ($w in $wins) {
  $tabs = $w.FindAll($TS::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::TabItem)))
  foreach ($t in $tabs) {
    $nm = $t.Current.Name
    Write-Host "  compare: name='$nm' like '*$expectedName*' = $($nm -like ('*' + $expectedName + '*'))"
    if ($nm -like ('*' + $expectedName + '*')) {
      $found = $true
      $docCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, $CT::Document)
      $doc = $w.FindFirst($TS::Descendants, $docCond)
      Write-Host "  found matching TabItem; document: $(if ($doc) {'FOUND, name=' + $doc.Current.Name + ', ct=' + $doc.Current.ControlType.ProgrammaticName} else {'NULL'})"
    }
  }
}
Write-Host "[5] overall found=$found"

# cleanup
Remove-Item -Path $path -Force -ErrorAction SilentlyContinue
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
