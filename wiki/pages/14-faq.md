---
title: FAQ
section: Project
summary: Short answers on safety, worldgen, mods, multiplayer, platforms and the relation to Horizon.
---

# FAQ

## Is Perforizon official?

No. It is an unofficial fan project, not affiliated with Mojang or Microsoft. It does not include Minecraft: you need your own copy of Minecraft Java 26.3, from which the game files are prepared on your machine. See [Credits](15-credits.md) for the disclaimer.

## Is it allowed?

Perforizon is a separate client. Whether a server accepts it is that server's decision: some servers run anticheats or rules against modified clients, and PvP overlays such as hitboxes or the time changer (see [PvP](07-pvp.md)) are the kind of thing servers restrict. Read the rules of the server you play on, and the Minecraft EULA. Perforizon cannot answer that for you.

## Is it safe?

- The source is public and MIT-licensed, and releases are built from it.
- Releases carry `SHA256SUMS` and a signature, `SHA256SUMS.sig`, checked by the launcher and by `horizon-release-sign`. See [Install](02-install.md).
- The one-line installer checks hashes only, not the signature. If that matters, use the launcher or verify the zip yourself.
- Do not run builds from sources you do not trust. The launcher refuses an unsigned release.

## Does it change world generation?

No. World generation is identical to Horizon's, so the same seed gives the same world. The Optimization Mode only decides whether rendering optimizations may change a few pixels; see [Parity mode](09-parity-mode.md).

## Is it really faster?

On the test machine, with default settings, at render distance 32: +5% to +44% average fps over Horizon built the same way, depending on the scene. It is one GPU and offscreen 720p, so read the tables in [Performance](08-performance.md) as a comparison between two builds. Then run the [auto-tune](04-auto-tune.md) on your hardware.

## Can I use Horizon mods?

Perforizon is a fork that tracks Horizon (see [Keeping up with Horizon](11-keeping-up-with-horizon.md)), and the launcher's Mods page offers the mods, shader packs and resource packs made for Horizon. Compatibility of each individual mod with Perforizon has not been checked here; if one fails, report it. Native mods are compiled by the launcher from their source.

## Can I join vanilla servers?

The client targets Minecraft Java 26.3 and speaks network protocol 777, which Perforizon does not change. It can join servers running that version. Servers on other versions are not supported. Joining online-mode servers needs a Microsoft sign-in in a launcher build that has it; the launcher's Settings page says when a build has none. Hit prediction and the crystal and anchor optimizers are untested on a live server.

## Does it need a Minecraft licence?

Yes: you need your own copy of Minecraft Java 26.3 to prepare the game files, and an account that owns it for online play.

## Which platforms are supported?

Windows x64 only today. The releases, the installer and the launcher asset are Windows packages. The source builds with Rust, but Linux and macOS packages are not made by this fork. See [Build from source](12-build-from-source.md).

## Why are most options off by default?

They were measured on one GPU and showed no gain there, or only within noise. The auto-tune turns on only what wins on your machine. See [Settings](05-settings.md).

## Where do I get help or report a bug?

[Troubleshooting](13-troubleshooting.md) lists symptoms, log locations and what to put in a report. Issues go to <https://github.com/Louchatfroff/Perforizon/issues>.
