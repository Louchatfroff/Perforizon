---
title: Use the Horizon launcher
section: Use
summary: Add a Perforizon instance to the original Horizon launcher with one script.
---

# Use the Horizon launcher

This is for people who want to keep using the Horizon launcher (for a video, a stream or just habit) but play
Perforizon. It is unofficial and not supported by Horizon. The launcher starts the `horizon-client.exe` of the game
version an instance is pinned to, so the script adds a **Perforizon** instance that plays its own version folder, made of a
copy of your Horizon version with Perforizon's executables on top. Your Horizon instance and version are not touched.

You need Windows x64, the original Horizon launcher with one version installed and prepared (press Play once), and your
own copy of Minecraft Java 26.3 as usual.

## Create the instance

[`tools/use-with-horizon-launcher.ps1`](https://github.com/Louchatfroff/Perforizon/blob/main/tools/use-with-horizon-launcher.ps1) does everything. Read it first, it is short.

```powershell
irm https://raw.githubusercontent.com/Louchatfroff/Perforizon/main/tools/use-with-horizon-launcher.ps1 -OutFile $env:TEMP\use-perforizon.ps1
powershell -ExecutionPolicy Bypass -File $env:TEMP\use-perforizon.ps1
```

Close the game first. The script:

1. finds the launcher's data folder (`%APPDATA%\horizon-launcher`, or `HORIZON_LAUNCHER_HOME`), the game home (`%APPDATA%\horizon`, or `HORIZON_HOME`) and your newest installed Horizon version;
2. downloads the latest Perforizon release zip and checks it against the release's `SHA256SUMS`;
3. copies that Horizon version (with its prepared game files) to `versions\perforizon` and puts Perforizon's `horizon-client.exe`, `horizon-ctl.exe` and `horizon-prepare.exe` in it;
4. creates the instance folder `instances\perforizon` in the game home, starting with your keys and video options (`options.txt`) and your offline name, and registers the instance `Perforizon`, pinned to that version, in `instances.json`.

Open the launcher: **Perforizon** is in the sidebar under Horizon. Select it and press **Play**. It has its own worlds,
options and packs. To bring a world over, copy its folder from the Horizon instance's `saves` into `instances\perforizon\saves` (world generation is identical, so it works in both).

Other commands (save the script and run it from its folder):

```powershell
.\use-with-horizon-launcher.ps1                      # run again to update Perforizon to the latest release
.\use-with-horizon-launcher.ps1 -Action Status       # what exists
.\use-with-horizon-launcher.ps1 -Action Remove       # remove the Perforizon version and instance entry (worlds are kept)
.\use-with-horizon-launcher.ps1 -Action Remove -DeleteWorlds   # ... and delete the instance folder too
.\use-with-horizon-launcher.ps1 -Version v2026.11.03 # copy from a specific installed Horizon version
.\use-with-horizon-launcher.ps1 -Zip C:\Downloads\Perforizon-windows-x64.zip   # a zip you already have
.\use-with-horizon-launcher.ps1 -LinkResources       # share the prepared game files with the Horizon version (saves disk space)
```

## Good to know

- **It is the game that changes, not the launcher.** The launcher still looks and behaves like Horizon's. Perforizon's options (auto-tune, latency counter, PvP) are inside the game: Options > Video Settings > Advanced Graphics, and Options > Horizon > PvP.
- **Disk space.** The Perforizon version holds its own copy of the prepared game files (hundreds of MB). `-LinkResources` shares them with the Horizon version instead, but then removing that Horizon version breaks the link: run the script again.
- **Prepared files.** Perforizon is built on Horizon's main of 2026-11-03. If the game says its files need preparing, press Prepare in the launcher on the Perforizon instance, or update your Horizon version and run the script again.
- **Updates.** The launcher never updates the Perforizon version by itself. Run the script again after a new Perforizon release.
- **Native (code) mods.** The launcher may rebuild the game when you install one for the Perforizon instance, which replaces Perforizon's executable with Horizon's build. Run the script again afterwards.
- **Not supported by the Horizon team.** If something breaks, remove the instance with `-Action Remove`, and report problems to Perforizon, not to Horizon.
