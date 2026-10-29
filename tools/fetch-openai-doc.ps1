#requires -Version 7.0

param(
    [Parameter(Mandatory = $true)]
    [string]$Url,

    [string]$Proxy = $(if ($env:HTTPS_PROXY) {
        $env:HTTPS_PROXY
    } elseif ($env:HTTP_PROXY) {
        $env:HTTP_PROXY
    } else {
        'http://127.0.0.1:30000'
    }),

    [ValidateRange(1, 120)]
    [int]$TimeoutSec = 30,

    [ValidateRange(1, 10)]
    [int]$ProxyAttempts = 3
)

$ErrorActionPreference = 'Stop'

$allowedHosts = @(
    'developers.openai.com',
    'platform.openai.com',
    'learn.chatgpt.com'
)

function Test-AllowedUri {
    param([System.Uri]$Uri)

    return $Uri.Scheme -eq 'https' -and $allowedHosts -contains $Uri.Host
}

function Invoke-DocumentRequest {
    param(
        [System.Uri]$Uri,
        [string]$Transport,
        [string]$ProxyUri
    )

    $request = @{
        Uri = $Uri
        Headers = @{ 'User-Agent' = 'Mozilla/5.0' }
        TimeoutSec = $TimeoutSec
        MaximumRedirection = 5
        UseBasicParsing = $true
    }
    if (-not [string]::IsNullOrWhiteSpace($ProxyUri)) {
        $request.Proxy = $ProxyUri
    }

    $response = Invoke-WebRequest @request
    $finalUri = [System.Uri]$response.BaseResponse.RequestMessage.RequestUri
    if (-not (Test-AllowedUri -Uri $finalUri)) {
        throw "redirected outside approved OpenAI documentation hosts: $finalUri"
    }
    if ([string]::IsNullOrWhiteSpace([string]$response.Content)) {
        throw "empty document body from $finalUri"
    }

    [Console]::Error.WriteLine(
        "openai-doc-fetch transport=$Transport status=$([int]$response.StatusCode) url=$finalUri bytes=$($response.Content.Length)"
    )
    return [string]$response.Content
}

$uri = [System.Uri]$Url
if (-not (Test-AllowedUri -Uri $uri)) {
    throw "URL must use HTTPS and one of: $($allowedHosts -join ', ')"
}

$directError = $null
try {
    $content = Invoke-DocumentRequest -Uri $uri -Transport 'direct' -ProxyUri ''
} catch {
    $directError = $_.Exception.Message
    [Console]::Error.WriteLine("openai-doc-fetch direct failed: $directError")
}

if ($null -eq $content) {
    $proxyUri = [System.Uri]$Proxy
    if ($proxyUri.Scheme -notin @('http', 'https')) {
        throw "proxy URL must use HTTP or HTTPS: $Proxy"
    }

    $lastError = $null
    for ($attempt = 1; $attempt -le $ProxyAttempts; $attempt++) {
        try {
            $content = Invoke-DocumentRequest -Uri $uri -Transport "proxy-attempt-$attempt" -ProxyUri $Proxy
            break
        } catch {
            $lastError = $_.Exception.Message
            [Console]::Error.WriteLine(
                "openai-doc-fetch proxy-attempt=$attempt failed: $lastError"
            )
            if ($attempt -lt $ProxyAttempts) {
                Start-Sleep -Seconds ([Math]::Min(4, $attempt))
            }
        }
    }
    if ($null -eq $content) {
        throw "direct failed ($directError); proxy failed after $ProxyAttempts attempts ($lastError)"
    }
}

[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::Out.Write($content)
