---
title: Troubleshooting
section: Project
summary: Symptoms and fixes: frame cap in the background, shader packs, drivers, logs and bug reports.
---

# Troubleshooting

## The game is stuck at a low frame rate when minimised or unfocused

Symptom: fps drops to a low cap (around 30) as soon as the window is minimised or loses focus, and benchmarks run in the background look slow.

Cause: the inactivity limiter, the vanilla option "Inactivity framerate". Its value is stored as `inactivityFpsLimit` in `options.txt`: `afk` (the default) or `minimized`.

Fixes:
- For normal play there is nothing to fix; it saves power. Bring the window to the front.
- For benchmarking or the auto-tune: `horizon-ctl perf-mode on` measures without a frame cap, VSync or the idle limit. `horizon-ctl perf-mode off` restores them.
- Unrelated to this: the game also pauses a window that loses focus for more than half a second, if the pause-on-lost-focus option is on.

## Low fps in general

1. Run the [auto-tune](04-auto-tune.md) in the world and at the render distance you play at.
2. Lower the render distance or the resolution scale: the GPU is the limit at high render distances.
3. Check `Optimization Mode`: in [parity mode](09-parity-mode.md) the pixel-changing optimizations are off.
4. Turn on `latencyHud` or the FPS overlay and compare before and after ([Latency](06-latency.md)).

## Shader pack problems

- With a shader pack, greedy meshing, BC7 texture compression and the late sky pass are switched off automatically, so fps differs from the benchmark tables.
- A shader that fails to compile does not crash the game: the compiler panic is caught. Remove the pack from the `shaderpacks` folder or pick another one.
- Rerun the auto-tune after changing shader packs. `shadowIndirect` is only tried while a pack draws a shadow map.
- Report it with the pack's name and version.

## Driver and GPU issues

- Update the graphics driver first.
- Mesh shaders (`meshShaders`) are off by default and slower on some GPUs. If you turned them on and the game misbehaves, set `horizon-ctl graphics meshShaders false` (or `HORIZON_MESH_SHADERS=0` in the environment).
- `graphicsApi` selects the graphics API if the default one is a problem on your machine.
- `pipelineCache` is Vulkan only and applies at the next start; turn it off if a driver rejects the cache.
- Visual artifacts in distant terrain: turn off `farLodCoarse`, or use [parity mode](09-parity-mode.md) to get Horizon's picture.

## The installer or the game does not start

See [Install](02-install.md): hash mismatches, the missing game files step, and where everything is installed (`%LOCALAPPDATA%\Perforizon`).

## Where the logs are

In the game folder (`HORIZON_HOME`, else the default game home):

- `logs/latest.log`, with the previous runs archived as `logs/<date>-<n>.log.gz`.
- `crash-reports/crash-<date>-client.txt` after a crash, with the error, the machine, the GPU, render settings and the last 50 log lines.
- `autotune.json`, the last auto-tune result.

`horizon-ctl logs [--lines N]` prints the end of `latest.log`, and `horizon-ctl logs --crash` the newest crash report. `horizon-ctl status` prints the current options.

## How to report

Open an issue at <https://github.com/Louchatfroff/Perforizon/issues> with:

- The Perforizon version (for example v0.1.0) and whether you installed with the launcher or the zip.
- Windows version, GPU and driver version.
- The Optimization Mode, your render distance, and the output of `horizon-ctl status`.
- `logs/latest.log` and, if any, the crash report.
- Steps to reproduce. If it looks like a rendering difference, try [parity mode](09-parity-mode.md) and say whether it goes away.

If the problem also exists in Horizon, it may belong upstream: <https://github.com/horizon-rust/horizon>. See [FAQ](14-faq.md).
