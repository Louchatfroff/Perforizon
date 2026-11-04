# Runs Perforizon from the original Horizon launcher by swapping the game executable of an installed version.
#   .\use-with-horizon-launcher.ps1                    install the latest Perforizon release into the newest installed version
#   .\use-with-horizon-launcher.ps1 -Action Status     show what is installed where
#   .\use-with-horizon-launcher.ps1 -Action Restore    put the original Horizon files back
#   .\use-with-horizon-launcher.ps1 -Zip C:\x\Perforizon-windows-x64.zip     use a zip you already downloaded
#   .\use-with-horizon-launcher.ps1 -Version v2026.11.03                    pick the installed version by tag
# Unofficial: see docs/use-with-horizon-launcher.md.
[CmdletBinding()]
param(
    [ValidateSet("Install", "Restore", "Status")]
    [string]$Action = "Install",
    [string]$Version,
    [string]$Zip,
    [string]$LauncherHome,
    [string]$Repo = "Louchatfroff/Perforizon"
)
$ErrorActionPreference = "Stop"
$Asset = "Perforizon-windows-x64.zip"
$Files = "horizon-client.exe", "horizon-ctl.exe"
$Marker = "perforizon-swapped.txt"

if (-not $LauncherHome) {
    $LauncherHome = $env:HORIZON_LAUNCHER_HOME
}
if (-not $LauncherHome) {
    $LauncherHome = Join-Path $env:APPDATA "horizon-launcher"
}
$Versions = Join-Path $LauncherHome "versions"
if (-not (Test-Path $Versions)) {
    throw "No versions folder at $Versions. Install a Horizon version with the original launcher first (or pass -LauncherHome)."
}

$Installed = Get-ChildItem $Versions -Directory |
    Where-Object { Test-Path (Join-Path $_.FullName ".installed") } |
    Sort-Object LastWriteTime -Descending
if (-not $Installed) {
    throw "No installed version found in $Versions. Press Play once in the original launcher so it installs one."
}

function Get-Target {
    if ($Version) {
        $hit = $Installed | Where-Object { $_.Name -eq $Version }
        if (-not $hit) { throw "Version $Version is not installed. Installed: $($Installed.Name -join ', ')" }
        return $hit
    }
    return $Installed | Select-Object -First 1
}

if ($Action -eq "Status") {
    foreach ($v in $Installed) {
        $state = if (Test-Path (Join-Path $v.FullName $Marker)) { "Perforizon" } else { "Horizon (original)" }
        "{0,-28} {1}" -f $v.Name, $state
    }
    return
}

$Target = Get-Target
$Dir = $Target.FullName

if (Get-Process -Name "horizon-client" -ErrorAction SilentlyContinue) {
    throw "The game is running. Close it first."
}

if ($Action -eq "Restore") {
    $restored = 0
    foreach ($f in $Files) {
        $orig = Join-Path $Dir "$f.orig"
        if (Test-Path $orig) {
            Move-Item -Force $orig (Join-Path $Dir $f)
            $restored++
        }
    }
    Remove-Item -Force (Join-Path $Dir $Marker) -ErrorAction SilentlyContinue
    if ($restored -eq 0) { "Nothing to restore in $($Target.Name)." } else { "Restored the original Horizon files in $($Target.Name)." }
    return
}

$Work = Join-Path ([IO.Path]::GetTempPath()) ("perforizon-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Force $Work | Out-Null
try {
    if ($Zip) {
        $ZipPath = (Resolve-Path $Zip).Path
    } else {
        $base = "https://github.com/$Repo/releases/latest/download"
        $ZipPath = Join-Path $Work $Asset
        "Downloading the latest Perforizon release from $Repo ..."
        Invoke-WebRequest "$base/$Asset" -OutFile $ZipPath -UseBasicParsing
        $SumsPath = Join-Path $Work "SHA256SUMS"
        Invoke-WebRequest "$base/SHA256SUMS" -OutFile $SumsPath -UseBasicParsing
        $want = (Get-Content $SumsPath | Where-Object { $_ -match [regex]::Escape($Asset) } | Select-Object -First 1) -replace "\s.*", ""
        $got = (Get-FileHash -Algorithm SHA256 $ZipPath).Hash.ToLowerInvariant()
        if (-not $want -or $want.ToLowerInvariant() -ne $got) {
            throw "The downloaded zip does not match SHA256SUMS. Nothing was changed."
        }
        "Checksum matches SHA256SUMS."
    }

    $Out = Join-Path $Work "out"
    Expand-Archive -Path $ZipPath -DestinationPath $Out -Force
    if (-not (Test-Path (Join-Path $Out "horizon-client.exe"))) {
        throw "The zip has no horizon-client.exe."
    }

    foreach ($f in $Files) {
        $src = Join-Path $Out $f
        if (-not (Test-Path $src)) { continue }
        $dst = Join-Path $Dir $f
        $orig = "$dst.orig"
        if ((Test-Path $dst) -and -not (Test-Path $orig)) {
            Copy-Item $dst $orig
        }
        Copy-Item -Force $src $dst
    }
    Set-Content -Path (Join-Path $Dir $Marker) -Value "Perforizon from $Repo, installed $(Get-Date -Format s)"
    "Perforizon is now the game in $($Target.Name)."
    "In the launcher: turn off 'Install new versions automatically' in Settings, then press Play."
    "To go back: .\use-with-horizon-launcher.ps1 -Action Restore"
} finally {
    Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
}
