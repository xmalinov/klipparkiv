param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Signature,
    [Parameter(Mandatory = $true)][string]$Url,
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Notes = ""
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Exe)) {
    throw "Update exe not found: $Exe"
}
if (-not (Test-Path $Signature)) {
    throw "Signature not found: $Signature. Run 'npx tauri signer sign --app-version $Version' first."
}

$hash = (Get-FileHash -Algorithm SHA256 -Path $Exe).Hash.ToLowerInvariant()
$size = (Get-Item $Exe).Length
$signatureText = [IO.File]::ReadAllText($Signature)
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$manifest = [ordered]@{
    schema  = 1
    channel = "stable"
    version = $Version
    pubDate = $pubDate
    notes   = $Notes
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            kind      = "exe"
            url       = $Url
            size      = $size
            sha256    = $hash
            signature = $signatureText
        }
    }
}

$json = $manifest | ConvertTo-Json -Depth 6
[IO.File]::WriteAllText($Out, $json, [Text.UTF8Encoding]::new($false))
Write-Host "Manifest: $Out"
Write-Host "SHA-256: $hash"
Write-Host "Size: $size bytes"
