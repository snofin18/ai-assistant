param(
    [Parameter(Mandatory = $true)][string]$CargoPath,
    [Parameter(Mandatory = $true)][string]$ReadyPath,
    [Parameter(Mandatory = $true)][string]$GoPath,
    [Parameter(Mandatory = $true)][string]$StdoutPath,
    [Parameter(Mandatory = $true)][string]$StderrPath
)
$ErrorActionPreference = "Stop"
[System.IO.File]::WriteAllText($ReadyPath, "ready")
while (-not (Test-Path -LiteralPath $GoPath)) {
    Start-Sleep -Milliseconds 20
}
$arguments = @(
    "test",
    "-p",
    "assistant-platform-windows",
    "test_windows_excel_real_com_repeated_write_acceptance",
    "--",
    "--ignored",
    "--nocapture",
    "--test-threads=1"
)
$child = Start-Process `
    -FilePath $CargoPath `
    -ArgumentList $arguments `
    -WindowStyle Hidden `
    -RedirectStandardOutput $StdoutPath `
    -RedirectStandardError $StderrPath `
    -PassThru
$child.WaitForExit()
exit $child.ExitCode
