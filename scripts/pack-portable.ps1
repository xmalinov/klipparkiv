param(
    [string]$OutDir = "dist-portable"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "src-tauri\target\release\klipparkiv.exe"
$helper = Join-Path $root "src-tauri\target\release\klipparkiv-updater.exe"
$bin = Join-Path $root "bin"
$out = Join-Path $root $OutDir

if (-not (Test-Path $exe)) {
    throw "Release exe not found: $exe. Run 'npm run build:portable' first."
}
if (-not (Test-Path $helper)) {
    throw "Updater helper not found: $helper. Run 'npm run build:portable' first."
}
if (-not (Test-Path $bin)) {
    throw "bin\ not found: $bin"
}

$models = Join-Path $bin "models"
$sourceModels = Join-Path $root "src-tauri\resources\bin\models"
if (Test-Path $sourceModels) {
    if (Test-Path $models) { Remove-Item $models -Recurse -Force }
    Copy-Item $sourceModels $models -Recurse -Force
} elseif (-not (Test-Path $models)) {
    throw "Beat grid models not found in bin\models or src-tauri\resources\bin\models"
}

if (Test-Path $out) { Remove-Item $out -Recurse -Force }
New-Item -ItemType Directory -Path $out | Out-Null

Copy-Item $exe (Join-Path $out "klipparkiv.exe")
Copy-Item $helper (Join-Path $out "klipparkiv-updater.exe")
Copy-Item $bin (Join-Path $out "bin") -Recurse
Copy-Item (Join-Path $root "README.txt") (Join-Path $out "README.txt")
Copy-Item (Join-Path $root "LICENSE") (Join-Path $out "LICENSE")
Copy-Item (Join-Path $root "THIRD_PARTY_NOTICES.md") (Join-Path $out "THIRD_PARTY_NOTICES.md")
Copy-Item (Join-Path $root "PRIVACY.md") (Join-Path $out "PRIVACY.md")
Copy-Item (Join-Path $root "SECURITY.md") (Join-Path $out "SECURITY.md")
Copy-Item (Join-Path $root "LICENSES") (Join-Path $out "LICENSES") -Recurse

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
$tar = Get-Command tar.exe -ErrorAction SilentlyContinue
if ($tar) {
    # Compress-Archive occasionally fails on large files held by antivirus.
    & $tar.Source -a -c -f $zip -C $out *
    if ($LASTEXITCODE -ne 0) { throw "tar failed with exit code $LASTEXITCODE" }
} else {
    Compress-Archive -Path (Join-Path $out "*") -DestinationPath $zip -CompressionLevel Optimal
}

$zipSize = (Get-Item $zip).Length / 1MB
Write-Host ("Archive: {0} ({1:N0} MB)" -f $zip, $zipSize)
