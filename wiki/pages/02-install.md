---
title: Install
section: Start
summary: Install Perforizon on Windows, verify it, prepare your game files, update and uninstall.
---

# Install

Perforizon v0.1.0 is packaged for Windows x64 only. You also need your own copy of Minecraft Java 26.3, because Perforizon contains none of Minecraft's code, textures or sounds.

## One-line install

In PowerShell, no administrator rights needed:

```powershell
irm https://raw.githubusercontent.com/Louchatfroff/Perforizon/main/packaging/windows/install-perforizon.ps1 | iex
```

The script downloads the latest release's `Perforizon-windows-x64.zip`, checks it against the release's `SHA256SUMS` (and the `SHA256SUMS` inside the archive), unpacks it to `%LOCALAPPDATA%\Perforizon` and adds a Start Menu shortcut.

Options: `-Version v0.1.0`, `-Dir D:\Perforizon`, `-Repo owner/name`.

The script checks integrity only. It does not verify `SHA256SUMS.sig`, so a release whose zip and sums were both replaced would pass. To authenticate a manual install, see the next sections.

## Zip install

Download from <https://github.com/Louchatfroff/Perforizon/releases>: `Perforizon-windows-x64.zip`, `SHA256SUMS` and `SHA256SUMS.sig`. The zip holds `horizon-client.exe`, `horizon-ctl.exe`, `horizon-prepare.exe`, `PLAY.txt`, `README.txt` and `SHA256SUMS`.

## Verify the signature

`horizon-release-sign` checks `SHA256SUMS.sig` and every file listed in `SHA256SUMS` against the keys built in from `crates/horizon-release-sign/release-keys.txt`. It is built from the source tree (see [Build from source](12-build-from-source.md)):

```sh
cargo run --profile fast -p horizon-release-sign -- verify --tag v0.1.0 <folder>
```

`<folder>` holds the downloaded release files. The tag must be the release's tag. The [launcher](10-launcher.md) performs the same signature check itself when it installs a release.

To check only the hash of a file: `Get-FileHash horizon-client.exe -Algorithm SHA256`, then compare it with `SHA256SUMS`.

## Prepare the game files

Do this once, from your own Minecraft Java 26.3 copy. `PLAY.txt` in the install folder has the details; the short form is:

```powershell
horizon-prepare.exe --prepare --server-jar PATH\TO\server.jar
horizon-prepare.exe sounds
```

Then run `horizon-client.exe` (`horizon-client.exe --help` lists the options). The launcher can also prepare the files, from the official launcher, Prism Launcher, MultiMC, the Modrinth App, ATLauncher or GDLauncher folder.

Next: [First launch](03-first-launch.md) and [Auto-tune](04-auto-tune.md).

## Update

Run the one-line installer again; it installs the latest release. Or use the launcher, which lists and installs releases of `Louchatfroff/Perforizon`. Saves and options live in the game folder, not in the install folder.

## Uninstall

1. Delete `%LOCALAPPDATA%\Perforizon` (or the folder you gave with `-Dir`).
2. Delete the Start Menu shortcut named Perforizon.
3. Optional: delete the game folder to remove saves, options and prepared game files. Back up `saves` first if you want to keep your worlds.

## Troubleshooting

- The installer fails to download: check the connection, or download the zip by hand and unzip it.
- The hash does not match: do not run the files; download again.
- The game does not start or shows placeholders: the game files were not prepared; repeat the preparation step.
- More in [Troubleshooting](13-troubleshooting.md).
