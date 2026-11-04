# Run Perforizon from the original Horizon launcher

This is for people who want to keep using the Horizon launcher (for a video, a stream or just habit) but play
Perforizon. It is unofficial and not supported by Horizon: the launcher starts whatever `horizon-client.exe` sits in
the folder of the installed version, so the trick is to put Perforizon's executable there and keep the original beside it.

You need Windows x64, the original Horizon launcher with one version installed and prepared (press Play once), and your
own copy of Minecraft Java 26.3 as usual.

## The quick way: the script

[`tools/use-with-horizon-launcher.ps1`](../tools/use-with-horizon-launcher.ps1) does everything below and can undo it. Read it first, it is short.

```powershell
irm https://raw.githubusercontent.com/Louchatfroff/Perforizon/main/tools/use-with-horizon-launcher.ps1 -OutFile $env:TEMP\use-perforizon.ps1
powershell -ExecutionPolicy Bypass -File $env:TEMP\use-perforizon.ps1
```

Close the game and the launcher's game window first. The script:

1. finds the launcher's data folder (`%APPDATA%\horizon-launcher`, or `HORIZON_LAUNCHER_HOME` if you set it) and the newest installed version;
2. downloads the latest Perforizon release zip and checks it against the release's `SHA256SUMS`;
3. renames the version's `horizon-client.exe` to `horizon-client.exe.orig` (and the same for `horizon-ctl.exe`), then copies Perforizon's files in;
4. writes `perforizon-swapped.txt` in that folder so it can tell which game is in place.

Other commands (save the script and run it from its folder):

```powershell
.\use-with-horizon-launcher.ps1 -Action Status                 # which installed version runs what
.\use-with-horizon-launcher.ps1 -Action Restore                # put the original Horizon back
.\use-with-horizon-launcher.ps1 -Version v2026.11.03           # a specific installed version
.\use-with-horizon-launcher.ps1 -Zip C:\Downloads\Perforizon-windows-x64.zip   # a zip you already have
```

Then, in the launcher, open Settings and turn off **Install new versions automatically** (otherwise the next Horizon
update rewrites the folder), and press **Play**.

## By hand

1. Install a Horizon version in the original launcher and press Play once so it is prepared. Close the game.
2. Download `Perforizon-windows-x64.zip` from the [Releases page](https://github.com/Louchatfroff/Perforizon/releases) and unzip it.
3. Open `%APPDATA%\horizon-launcher\versions\<tag>\` (the newest tag folder, the one with a `.installed` file).
4. Rename `horizon-client.exe` to `horizon-client.exe.orig`. Copy `horizon-client.exe` (and `horizon-ctl.exe`) from the zip into the folder.
5. Settings: turn off **Install new versions automatically**. Press Play.

To go back, delete the new `horizon-client.exe` and rename the `.orig` file back.

## Good to know

- **It is the game that changes, not the launcher.** The launcher still looks and behaves like Horizon's. Perforizon's options (auto-tune, latency counter, PvP) are inside the game: Options > Video Settings > Advanced Graphics, and Options > Horizon > PvP.
- **Prepared files.** Perforizon is built on Horizon's main of 2026-11-03. If the game says its game files need preparing, press Prepare in the launcher (or update the Horizon version, then run the script again).
- **Updates.** A Horizon update installs a new version folder; run the script again to swap it, or use `-Version`.
- **Native (code) mods.** The launcher may rebuild the game when you install one; that replaces Perforizon with Horizon's build. Run the script again afterwards.
- **Worlds and settings are shared** with your Horizon install (same data folder), and world generation is identical, so worlds work in both. For a clean side-by-side comparison give each its own `HORIZON_HOME` folder, see [the benchmark guide](perforizon-benchmarks.md).
- **Not supported by the Horizon team.** If something breaks, restore first, and report problems to Perforizon, not to Horizon.
