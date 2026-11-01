param(
    [string]$Exe = "dist-portable\klipparkiv.exe",
    [string]$Out = "docs\mockups\05-m1-download.png",
    [int]$Port = 9222,
    [string]$Url = "",
    [string]$Mode = "audio",
    [string]$Quality = "720",
    [string]$ClipStart = "",
    [string]$ClipEnd = "",
    [switch]$NoSubmit,
    [string]$Js = "",
    [int]$CancelAfter = 0,
    [int]$Wait = 4
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$exePath = if ([System.IO.Path]::IsPathRooted($Exe)) { $Exe } else { Join-Path $root $Exe }
$outPath = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $root $Out }
if (-not (Test-Path $exePath)) { throw "Missing $exePath" }

function Send-Cdp {
    param($Socket, $Id, $Method, $Params)
    $payload = @{ id = $Id; method = $Method; params = $Params } |
        ConvertTo-Json -Depth 8 -Compress
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
    $token = [System.Threading.CancellationToken]::None
    $Socket.SendAsync([ArraySegment[byte]]::new($bytes), "Text", $true, $token).Wait()

    while ($true) {
        $buffer = New-Object byte[] 1048576
        $stream = New-Object System.IO.MemoryStream
        do {
            $segment = [ArraySegment[byte]]::new($buffer)
            $result = $Socket.ReceiveAsync($segment, $token).Result
            $stream.Write($buffer, 0, $result.Count)
        } while (-not $result.EndOfMessage)

        $message = [System.Text.Encoding]::UTF8.GetString($stream.ToArray()) | ConvertFrom-Json
        if ($message.id -eq $Id) { return $message }
    }
}

$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$process = Start-Process -FilePath $exePath -PassThru

try {
    $page = $null
    $deadline = (Get-Date).AddSeconds(20)
    while (-not $page -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 500
        try {
            $targets = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/list" -TimeoutSec 2
            $page = $targets | Where-Object { $_.type -eq "page" } | Select-Object -First 1
        } catch { }
    }
    if (-not $page) { throw "WebView2 debug target not found on port $Port" }

    $socket = [System.Net.WebSockets.ClientWebSocket]::new()
    $token = [System.Threading.CancellationToken]::None
    $socket.ConnectAsync([Uri]$page.webSocketDebuggerUrl, $token).Wait()

    if ($Js) {
        Start-Sleep -Seconds 3
        $result = Send-Cdp -Socket $socket -Id 5 -Method "Runtime.evaluate" -Params @{
            expression = $Js
            awaitPromise = $true
            returnByValue = $true
        }
        if ($result.result.exceptionDetails) {
            $description = $result.result.exceptionDetails.exception.description
            throw "Page script failed: $description"
        }
        if ($result.result.result.value) {
            Write-Host "JS: $($result.result.result.value)"
        }
    }

    if ($Url) {
        $modeStep = switch ($Mode) {
            "video" { "document.querySelectorAll('[role=tab]')[1].click();" }
            "both" { "document.querySelectorAll('[role=tab]')[2].click();" }
            "clip" { "document.querySelectorAll('[role=tab]')[3].click();" }
            default { "" }
        }
        $clipStep = if ($Mode -eq "clip" -and $ClipStart -and $ClipEnd) {
            @"
    const clips = document.querySelectorAll('.field.clip');
    if (clips.length === 2) {
      const clipSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
      clipSetter.call(clips[0], '$ClipStart');
      clips[0].dispatchEvent(new Event('input', { bubbles: true }));
      clipSetter.call(clips[1], '$ClipEnd');
      clips[1].dispatchEvent(new Event('input', { bubbles: true }));
    }
"@
        } else {
            ""
        }
        $delay = if ($Mode -eq "audio") { 0 } else { 300 }
        $submitStep = if ($NoSubmit) {
            ""
        } else {
            "setTimeout(() => document.querySelector('.btn.primary').click(), 250);"
        }
        $expression = @"
(() => {
  $modeStep
  setTimeout(() => {
    const selects = document.querySelectorAll('.quality select');
    const select = selects[selects.length - 1];
    if (select) {
      const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set;
      setter.call(select, '$Quality');
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }
    $clipStep
    const input = document.querySelector('input');
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    setter.call(input, '$Url');
    input.dispatchEvent(new Event('input', { bubbles: true }));
    $submitStep
  }, $delay);
  return 'queued';
})()
"@
        $result = Send-Cdp -Socket $socket -Id 1 -Method "Runtime.evaluate" -Params @{
            expression = $expression
            returnByValue = $true
        }
        if ($result.result.exceptionDetails) {
            $description = $result.result.exceptionDetails.exception.description
            throw "Page script failed: $description"
        }
        if ($result.result.result.value) {
            Write-Host "JS: $($result.result.result.value)"
        }

        if ($CancelAfter -gt 0) {
            Start-Sleep -Seconds $CancelAfter
            Send-Cdp -Socket $socket -Id 10 -Method "Runtime.evaluate" -Params @{
                expression = "document.querySelector('.icon-btn').click(); 'canceled'"
            } | Out-Null
            Start-Sleep -Seconds 3
            $orphans = @(Get-Process -Name "yt-dlp","ffmpeg" -ErrorAction SilentlyContinue).Count
            Write-Host "yt-dlp/ffmpeg processes after cancel: $orphans"
        }
    }

    Start-Sleep -Seconds $Wait

    $shot = Send-Cdp -Socket $socket -Id 2 -Method "Page.captureScreenshot" -Params @{
        format = "png"
        captureBeyondViewport = $false
    }
    if (-not $shot.result.data) { throw "No screenshot data" }

    $png = [Convert]::FromBase64String($shot.result.data)
    [System.IO.File]::WriteAllBytes($outPath, $png)
    $socket.Dispose()

    Write-Host "Saved $outPath ($([math]::Round($png.Length / 1KB)) KB)"
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}
