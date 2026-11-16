param(
    [Parameter(Mandatory = $true)][string]$Marker,
    [switch]$Child
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($Marker)) {
    throw "Marker must be non-empty"
}

if (-not $Child) {
    $pwshPath = (Get-Command pwsh -ErrorAction Stop).Source
    Start-Process `
        -FilePath $pwshPath `
        -ArgumentList @(
            "-NoProfile",
            "-File",
            $PSCommandPath,
            "-Marker",
            $Marker,
            "-Child"
        ) `
        -WindowStyle Hidden | Out-Null
}

Start-Sleep -Seconds 60
