# Adds a "Perforizon" instance to the original Horizon launcher, next to the Horizon one, which stays untouched.
#   .\use-with-horizon-launcher.ps1                    create (or update) the Perforizon instance from the latest release
#   .\use-with-horizon-launcher.ps1 -Action Status     show what exists
#   .\use-with-horizon-launcher.ps1 -Action Remove     remove the Perforizon version and instance entry (worlds are kept)
#   .\use-with-horizon-launcher.ps1 -Zip C:\x\Perforizon-windows-x64.zip   use a zip you already downloaded
#   .\use-with-horizon-launcher.ps1 -Version v2026.11.03    copy files from this installed Horizon version
#   .\use-with-horizon-launcher.ps1 -LinkResources     share the prepared game files with the Horizon version instead of copying them
# Unofficial: see docs/use-with-horizon-launcher.md.
[CmdletBinding()]
param(
    [ValidateSet("Install", "Remove", "Status")]
    [string]$Action = "Install",
    [string]$Version,
    [string]$Zip,
    [string]$LauncherHome,
    [string]$GameHome,
    [string]$Name = "Perforizon",
    [switch]$LinkResources,
    [switch]$DeleteWorlds,
    [string]$Repo = "Louchatfroff/Perforizon"
)
$ErrorActionPreference = "Stop"
$Asset = "Perforizon-windows-x64.zip"
$Slug = "perforizon"
$Tag = "perforizon"

if (-not $LauncherHome) { $LauncherHome = $env:HORIZON_LAUNCHER_HOME }
if (-not $LauncherHome) { $LauncherHome = Join-Path $env:APPDATA "horizon-launcher" }
if (-not $GameHome) { $GameHome = $env:HORIZON_HOME }
if (-not $GameHome) { $GameHome = Join-Path $env:APPDATA "horizon" }
$Versions = Join-Path $LauncherHome "versions"
$Store = Join-Path $LauncherHome "instances.json"
$InstanceDir = Join-Path (Join-Path $GameHome "instances") $Slug
$NewVersion = Join-Path $Versions $Tag

function Read-Store {
    if (Test-Path $Store) {
        $raw = Get-Content $Store -Raw
        if ($raw.Trim()) { return $raw | ConvertFrom-Json }
    }
    return [pscustomobject]@{ instances = [pscustomobject]@{} }
}
function Write-Store($obj) {
    New-Item -ItemType Directory -Force $LauncherHome | Out-Null
    $json = $obj | ConvertTo-Json -Depth 20
    [IO.File]::WriteAllText($Store, $json, (New-Object Text.UTF8Encoding $false))
}
function Get-Installed {
    if (-not (Test-Path $Versions)) { return @() }
    return @(Get-ChildItem $Versions -Directory |
        Where-Object { $_.Name -ne $Tag -and (Test-Path (Join-Path $_.FullName ".installed")) } |
        Sort-Object LastWriteTime -Descending)
}

if ($Action -eq "Status") {
    "Launcher data : $LauncherHome"
    "Game home     : $GameHome"
    $inst = Get-Installed
    "Horizon versions: " + $(if ($inst) { $inst.Name -join ", " } else { "none" })
    $has = (Test-Path (Join-Path $NewVersion ".installed"))
    "Perforizon version : " + $(if ($has) { "installed ($NewVersion)" } else { "not installed" })
    $s = Read-Store
    $listed = $s.instances.PSObject.Properties.Name -contains $Slug
    "Perforizon instance: " + $(if ($listed -and (Test-Path $InstanceDir)) { "present ($InstanceDir)" } else { "not present" })
    return
}

if (Get-Process -Name "horizon-client" -ErrorAction SilentlyContinue) {
    throw "The game is running. Close it first."
}

if ($Action -eq "Remove") {
    if (Test-Path $NewVersion) {
        $local = Join-Path $NewVersion ".local"
        if ((Test-Path $local) -and ((Get-Item $local -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            (Get-Item $local -Force).Delete()
        }
        Remove-Item -Recurse -Force $NewVersion
        "Removed the Perforizon version."
    }
    if (Test-Path $Store) {
        $s = Read-Store
        if ($s.instances.PSObject.Properties.Name -contains $Slug) {
            $s.instances.PSObject.Properties.Remove($Slug)
            Write-Store $s
        }
    }
    if ($DeleteWorlds -and (Test-Path $InstanceDir)) {
        Remove-Item -Recurse -Force $InstanceDir
        "Deleted the instance folder with its worlds."
    } elseif (Test-Path $InstanceDir) {
        "The instance folder (worlds, options) is kept: $InstanceDir. Add -DeleteWorlds to delete it."
    }
    return
}

$Base = $null
$Installed = Get-Installed
if (-not $Installed) {
    throw "No Horizon version is installed in $Versions. Press Play once in the original launcher so it installs and prepares one."
}
if ($Version) {
    $Base = $Installed | Where-Object { $_.Name -eq $Version }
    if (-not $Base) { throw "Version $Version is not installed. Installed: $($Installed.Name -join ', ')" }
} else {
    $Base = $Installed | Select-Object -First 1
}

$Work = Join-Path ([IO.Path]::GetTempPath()) ("perforizon-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Force $Work | Out-Null
try {
    if ($Zip) {
        $ZipPath = (Resolve-Path $Zip).Path
    } else {
        $url = "https://github.com/$Repo/releases/latest/download"
        $ZipPath = Join-Path $Work $Asset
        "Downloading the latest Perforizon release from $Repo ..."
        Invoke-WebRequest "$url/$Asset" -OutFile $ZipPath -UseBasicParsing
        $SumsPath = Join-Path $Work "SHA256SUMS"
        Invoke-WebRequest "$url/SHA256SUMS" -OutFile $SumsPath -UseBasicParsing
        $want = (Get-Content $SumsPath | Where-Object { $_ -match [regex]::Escape($Asset) } | Select-Object -First 1) -replace "\s.*", ""
        $got = (Get-FileHash -Algorithm SHA256 $ZipPath).Hash.ToLowerInvariant()
        if (-not $want -or $want.ToLowerInvariant() -ne $got) {
            throw "The downloaded zip does not match SHA256SUMS. Nothing was changed."
        }
        "Checksum matches SHA256SUMS."
    }
    $Out = Join-Path $Work "out"
    Expand-Archive -Path $ZipPath -DestinationPath $Out -Force
    if (-not (Test-Path (Join-Path $Out "horizon-client.exe"))) { throw "The zip has no horizon-client.exe." }

    "Creating the Perforizon version from $($Base.Name) ..."
    if (Test-Path $NewVersion) {
        $oldLocal = Join-Path $NewVersion ".local"
        if ((Test-Path $oldLocal) -and ((Get-Item $oldLocal -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            (Get-Item $oldLocal -Force).Delete()
        }
        Remove-Item -Recurse -Force $NewVersion
    }
    New-Item -ItemType Directory -Force $NewVersion | Out-Null
    $null = robocopy $Base.FullName $NewVersion /E /XD ".local" /NFL /NDL /NJH /NJS /NP
    if ($LASTEXITCODE -ge 8) { throw "Copying the Horizon version failed (robocopy $LASTEXITCODE)." }
    $baseLocal = Join-Path $Base.FullName ".local"
    if (Test-Path $baseLocal) {
        $newLocal = Join-Path $NewVersion ".local"
        if ($LinkResources) {
            New-Item -ItemType Junction -Path $newLocal -Target $baseLocal | Out-Null
        } else {
            $null = robocopy $baseLocal $newLocal /E /NFL /NDL /NJH /NJS /NP
            if ($LASTEXITCODE -ge 8) { throw "Copying the prepared game files failed (robocopy $LASTEXITCODE)." }
        }
    }
    foreach ($f in "horizon-client.exe", "horizon-ctl.exe", "horizon-prepare.exe") {
        $src = Join-Path $Out $f
        if (Test-Path $src) { Copy-Item -Force $src (Join-Path $NewVersion $f) }
    }
    Set-Content -Path (Join-Path $NewVersion ".installed") -Value ""
    Set-Content -Path (Join-Path $NewVersion "perforizon.txt") -Value "Perforizon from $Repo, based on $($Base.Name), $(Get-Date -Format s)"

    New-Item -ItemType Directory -Force $InstanceDir | Out-Null
    foreach ($f in "options.txt", "horizon-player.txt") {
        $from = Join-Path $GameHome $f
        $to = Join-Path $InstanceDir $f
        if ((Test-Path $from) -and -not (Test-Path $to)) { Copy-Item $from $to }
    }

    $s = Read-Store
    if (-not ($s.PSObject.Properties.Name -contains "instances")) {
        $s | Add-Member -NotePropertyName instances -NotePropertyValue ([pscustomobject]@{})
    }
    $meta = [pscustomobject]@{ name = $Name; version = $Tag }
    if ($s.instances.PSObject.Properties.Name -contains $Slug) {
        $existing = $s.instances.$Slug
        $existing | Add-Member -NotePropertyName name -NotePropertyValue $Name -Force
        $existing | Add-Member -NotePropertyName version -NotePropertyValue $Tag -Force
    } else {
        $s.instances | Add-Member -NotePropertyName $Slug -NotePropertyValue $meta
    }
    Write-Store $s

    ""
    "Done. Open the original launcher: there is a '$Name' instance in the sidebar."
    "Select it and press Play. The Horizon instance and its version are unchanged."
    "Run this script again to update Perforizon; -Action Remove takes it away."
} finally {
    Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
}
