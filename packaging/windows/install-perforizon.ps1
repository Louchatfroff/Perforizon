# Installs Perforizon for the current user (no admin): downloads the latest release of Louchatfroff/Perforizon,
# checks it against the release's SHA256SUMS (integrity only: SHA256SUMS.sig is NOT verified here, the launcher does
# that; compare the printed SHA-256 with the one published by the author), unpacks to %LOCALAPPDATA%\Perforizon and adds a Start Menu shortcut.
#
#   irm https://raw.githubusercontent.com/Louchatfroff/Perforizon/main/packaging/windows/install-perforizon.ps1 | iex
#   & ([scriptblock]::Create((irm .../install-perforizon.ps1))) -Version v0.1.0 -Dir D:\Perforizon
#
# Prepared game files (your own Minecraft) live in the game folder, not here, so updating keeps them.
param(
    [string]$Version = "latest",
    [string]$Dir = (Join-Path $env:LOCALAPPDATA "Perforizon"),
    [string]$Repo = "Louchatfroff/Perforizon"
)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$Asset = "Perforizon-windows-x64.zip"
if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") { throw "Perforizon is built for 64-bit Windows (x64) only" }

$Api = "https://api.github.com/repos/$Repo/releases"
$Url = if ($Version -eq "latest") { "$Api/latest" } else { "$Api/tags/$Version" }
$Release = Invoke-RestMethod -Uri $Url -Headers @{ Accept = "application/vnd.github+json" }
$Tag = $Release.tag_name
$Item = $Release.assets | Where-Object { $_.name -eq $Asset } | Select-Object -First 1
$SumsItem = $Release.assets | Where-Object { $_.name -eq "SHA256SUMS" } | Select-Object -First 1
if (-not $Item) { throw "Release $Tag has no $Asset" }
if (-not $SumsItem) { throw "Release ${Tag} has no SHA256SUMS: not installed" }

$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("perforizon-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    $SumsFile = Join-Path $Tmp "SHA256SUMS"
    Invoke-WebRequest -Uri $SumsItem.browser_download_url -OutFile $SumsFile
    $Expected = $null
    foreach ($l in Get-Content $SumsFile) {
        if ($l -match "^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$" -and $Matches[2] -eq $Asset) { $Expected = $Matches[1].ToLowerInvariant() }
    }
    if (-not $Expected) { throw "SHA256SUMS lists no ${Asset}: not installed" }

    Write-Host "Downloading Perforizon $Tag..."
    $Zip = Join-Path $Tmp $Asset
    Invoke-WebRequest -Uri $Item.browser_download_url -OutFile $Zip
    $Got = (Get-FileHash -Algorithm SHA256 $Zip).Hash.ToLowerInvariant()
    Write-Host "SHA-256 of ${Asset}: ${Got}"
    if ($Got -ne $Expected) { throw "${Asset} does not match SHA256SUMS (got ${Got}, expected ${Expected}): not installed" }

    $Unpacked = Join-Path $Tmp "x"
    Expand-Archive -Path $Zip -DestinationPath $Unpacked
    $Inner = Join-Path $Unpacked "SHA256SUMS"
    if (Test-Path $Inner) {
        foreach ($l in Get-Content $Inner) {
            if ($l -match "^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$") {
                $Name = $Matches[2]
                $Hash = $Matches[1].ToLowerInvariant()
                $File = Join-Path $Unpacked $Name
                if (-not (Test-Path $File) -or (Get-FileHash -Algorithm SHA256 $File).Hash.ToLowerInvariant() -ne $Hash) {
                    throw "${Name} does not match the SHA256SUMS inside the archive: not installed"
                }
            }
        }
    }
    if (-not (Test-Path (Join-Path $Unpacked "horizon-client.exe"))) { throw "${Asset} holds no horizon-client.exe" }

    New-Item -ItemType Directory -Force $Dir | Out-Null
    Get-ChildItem $Unpacked -File | ForEach-Object { Copy-Item $_.FullName (Join-Path $Dir $_.Name) -Force }
    Set-Content -Path (Join-Path $Dir "VERSION") -Value $Tag -Encoding ASCII

    $Menu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
    $Shell = New-Object -ComObject WScript.Shell
    $Link = $Shell.CreateShortcut((Join-Path $Menu "Perforizon.lnk"))
    $Link.TargetPath = Join-Path $Dir "horizon-client.exe"
    $Link.WorkingDirectory = $Dir
    $Link.Description = "Perforizon"
    $Link.Save()
    Write-Host "Installed Perforizon ${Tag} to ${Dir} and added a Start Menu shortcut."
    Write-Host "First run: read $Dir\PLAY.txt to prepare the game files from your own Minecraft."
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
