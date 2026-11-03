# Builds Perforizon and packs dist/Perforizon-windows-x64.zip (docs/perforizon-install.md).
#   tools\package_windows.ps1                    release profile
#   tools\package_windows.ps1 -Profile fast      the quicker, non-LTO profile
#   tools\package_windows.ps1 -StaticCrt         link the C runtime in (no Visual C++ redistributable needed; rebuilds everything)
#   tools\package_windows.ps1 -SkipBuild         pack what target\<profile> already holds
param(
    [string]$Profile = "release",
    [switch]$StaticCrt,
    [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$Asset = "Perforizon-windows-x64.zip"
$Bins = "horizon-client.exe", "horizon-ctl.exe", "horizon-prepare.exe"

if (-not $SkipBuild) {
    if ($StaticCrt) { $env:RUSTFLAGS = "-C target-feature=+crt-static" }
    cargo build --profile $Profile --locked -p horizon-client --bin horizon-client --bin horizon-ctl -p horizon-prepare --bin horizon-prepare
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$Out = Join-Path $Root "target\$Profile"
$Stage = Join-Path $Root "dist\Perforizon-windows-x64"
$Zip = Join-Path $Root "dist\$Asset"
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force $Stage | Out-Null
foreach ($b in $Bins) {
    $src = Join-Path $Out $b
    if (-not (Test-Path $src)) { throw "$src is missing (build first, or pass the right -Profile)" }
    Copy-Item $src $Stage
}
Copy-Item (Join-Path $Root "packaging\PLAY.txt") (Join-Path $Stage "PLAY.txt")
Copy-Item (Join-Path $Root "packaging\windows\PERFORIZON-README.txt") (Join-Path $Stage "README.txt")
Copy-Item (Join-Path $Root "LICENSE") $Stage -ErrorAction SilentlyContinue

$Utf8 = New-Object Text.UTF8Encoding $false
$Lines = Get-ChildItem $Stage -File | Sort-Object Name | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLowerInvariant(), $_.Name
}
[IO.File]::WriteAllText((Join-Path $Stage "SHA256SUMS"), (($Lines -join "`n") + "`n"), $Utf8)

if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip

$ZipHash = (Get-FileHash -Algorithm SHA256 $Zip).Hash.ToLowerInvariant()
[IO.File]::WriteAllText((Join-Path $Root "dist\SHA256SUMS"), "$ZipHash  $Asset`n", $Utf8)
Write-Host "Wrote $Zip ($([math]::Round((Get-Item $Zip).Length / 1MB, 1)) MB)"
Write-Host "Wrote dist\SHA256SUMS (upload both to the release; sign it as docs/perforizon-install.md says)"
