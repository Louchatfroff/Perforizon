---
title: Home
section: Start
summary: What Perforizon is, three facts worth knowing, and where to go next.
---

# Perforizon

Perforizon is a performance fork of [Horizon](https://github.com/horizon-rust/horizon), which is Minecraft: Java Edition 26.3 rewritten in Rust on top of wgpu. It goes after frame rate and input latency and leaves everything else alone.

It is unofficial and is not affiliated with Mojang or Microsoft. It does not include Minecraft: you need your own copy of Minecraft Java 26.3, and the game files are prepared on your machine from it. Horizon (MIT) is its base; see [Credits](15-credits.md).

## Three facts

1. **World generation is identical to Horizon's.** The same seed gives the same world. Game logic is unchanged. See the [FAQ](14-faq.md).
2. **It is faster than Horizon with default settings.** Against Horizon built the same way, at render distance 32 on one test machine (RTX 4060 Ti, offscreen 720p, average of 3 runs), average fps is +5% in village, +44% in forest, +26% in ocean and +6% in mountain. The 1% lows improve by 16% to 35%. The full tables, with their caveats, are in [Performance](08-performance.md).
3. **A built-in auto-tune measures your machine and keeps what is faster.** The quick one takes about two minutes; the precise one takes tens of minutes. See [Auto-tune](04-auto-tune.md).

One machine is not a promise for yours. Run the auto-tune once per machine and once after changing render distance.

## What you get

- Windows x64 releases, signed; the current release is v0.1.0. Other platforms are not packaged yet.
- Two modes: Performance (the default) and 100% parity, which turns off every optimization that changes pixels. See [Parity mode](09-parity-mode.md).
- A PvP module: hit prediction, optimizers, hitboxes and HUD overlays. See [PvP](07-pvp.md).
- An input-to-screen latency counter and latency-aware present settings. See [Latency](06-latency.md).
- A terminal-style launcher with sixteen colour palettes. See [Launcher](10-launcher.md).

## Where to go next

- New here: [Install](02-install.md), then [First launch](03-first-launch.md).
- Want frames: [Auto-tune](04-auto-tune.md), then [Settings](05-settings.md).
- Want to know why it is faster: [Performance](08-performance.md).
- Something is wrong: [Troubleshooting](13-troubleshooting.md).
- Want to build it: [Build from source](12-build-from-source.md).
- How it follows Horizon: [Keeping up with Horizon](11-keeping-up-with-horizon.md).

## Links

- Source and releases: <https://github.com/Louchatfroff/Perforizon>
- Upstream: <https://github.com/horizon-rust/horizon>
