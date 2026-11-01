param(
    [string]$OutDir = "dist-portable"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "src-tauri\target\release\klipparkiv.exe"
$bin = Join-Path $root "bin"
$out = Join-Path $root $OutDir

if (-not (Test-Path $exe)) {
    throw "Release exe not found: $exe. Run 'npm run build:portable' first."
}
if (-not (Test-Path $bin)) {
    throw "bin\ not found: $bin"
}

if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory -Path $out | Out-Null

Copy-Item $exe (Join-Path $out "klipparkiv.exe")
Copy-Item $bin (Join-Path $out "bin") -Recurse
Copy-Item (Join-Path $root "README.txt") (Join-Path $out "README.txt")

foreach ($dir in "data", "audio", "video") {
    New-Item -ItemType Directory -Path (Join-Path $out $dir) | Out-Null
}

$size = (Get-ChildItem $out -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
Write-Host ("Portable build: {0} ({1:N0} MB)" -f $out, $size)

$version = (Get-Content (Join-Path $root "package.json") -Raw | ConvertFrom-Json).version
$release = Join-Path $root "release"
New-Item -ItemType Directory -Path $release -Force | Out-Null
$zip = Join-Path $release ("klipparkiv-{0}-win-x64.zip" -f $version)
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $out "*") -DestinationPath $zip -CompressionLevel Optimal

$zipSize = (Get-Item $zip).Length / 1MB
Write-Host ("Archive: {0} ({1:N0} MB)" -f $zip, $zipSize)
