---
title: Auto-tune
section: Use
summary: Quick and precise auto-tune, Optimization Mode, what is tried and how to read the result.
---

# Auto-tune

The auto-tune measures your machine and keeps the graphics settings that run best on it. Perforizon's defaults are already faster than Horizon's; the auto-tune handles the options that depend on your GPU. The option reference is in [Settings](05-settings.md).

## 1. Pick an Optimization Mode

`Options > Video Settings > Advanced Graphics > Auto-tune > Optimization Mode`

| Mode | What it does |
| --- | --- |
| Performance (default) | Allows every optimization that changes a few pixels: greedy meshing, solid distant leaves, merged water faces. The auto-tune may also lower the resolution scale. |
| 100% parity | Turns off every optimization that changes pixels. Only lossless speedups stay. The auto-tune never lowers the resolution scale. |

Game logic and world generation are untouched in both. Details: [Parity mode](09-parity-mode.md). From the command line: `horizon-ctl graphics optimizationMode performance` or `parity`.

## 2. Quick auto-tune

1. Open a creative world (or your single-player world) and stand where you normally play, at your usual render distance. The tune runs in the world you are in, so the loaded terrain is what gets measured.
2. Go to `Options > Video Settings > Advanced Graphics > Auto-tune > Run Auto-tune...`.
3. The options screen closes, the camera moves for a short measurement, and the game restores your position and state afterwards. It takes up to about two minutes (a hard cap of 110 s of measuring, plus remesh waits).
4. A chat notice ends it: `Auto-tune: done in Ns, X fps (1% low Y); kept key=value, ...`.
5. The result is saved to your graphics options and persists across restarts.

Command line, with a world open:

```sh
horizon-ctl autotune 600     # target fps; below it the resolution scale may be lowered (Performance only)
horizon-ctl status           # shows the result and the current options
```

### What it tries

In order, keeping a candidate only if it beats the current choice by more than 3%:

1. Mesh shaders (off unless faster), cave culling, entity culling, shadow cache, greedy meshing.
2. In Performance mode: `leafDistance` (5, 3, 0), then `farTerrainDistance` and `farLodCoarse` together as one step (off, 16 chunks coarse, 10 chunks coarse).
3. The resolution scale, only if you asked for a target your machine does not reach.
4. The latency steps, `presentMode` and `frameLatency`, which change no pixels and also run in 100% parity. See [Latency](06-latency.md).

After each change that remeshes terrain it waits for the chunks to settle before measuring.

## 3. Precise auto-tune

Slower and more careful. It measures the same four scenes as the benchmark (village, forest, ocean, mountain, seed 1234), several passes per candidate.

- Start it with `Run Precise Auto-tune...` in the same menu, or `horizon-ctl autotune --precise [FPS [PASSES]]` (PASSES defaults to 3).
- Stop it by clicking the button again or with `horizon-ctl autotune --cancel`. A cancelled or failed run restores your graphics options.
- The scenes exist only in the benchmark world (`saves/horizon-benchmark`, replaced on every run). The run saves and leaves your world, and ends at the title screen: reopen your world from the list. It is not available while connected to a remote server.
- Progress appears as chat notices, for example `Precise auto-tune: 5/15 hizOcclusion=true: -6.5% (needs 9.7%)`.
- A candidate is kept only if it beats the current choice by more than the measured noise (never below 2%). Ties go to the earlier value in the list.
- A final validation measures the defaults and the chosen set again; if the chosen set does not score higher, the defaults are restored.
- Time: about 100 s per candidate on the reference machine, about 32 minutes for 15 candidates. A 90 minute budget ends the search early.

Reference run (RTX 4060 Ti, render distance 32, Performance mode, 3 passes): 1930 s, kept `farLod=10c` only. The baseline score was 587 and the chosen set 864; validation passed.

## 4. Reading the result

The result goes to `autotune.json` in the game folder: `mode`, per-scene averages and lows of every try, pass scores, noise, validation, `kept` and `seconds`. `horizon-ctl status` shows the current options.

Most options stay off on the reference GPU because they showed no gain there; that is the expected outcome, not a failed tune.

## When to rerun

- Once per machine.
- After changing render distance.
- After changing shader packs.
- After a GPU or driver change.

The GPU is the limit at high render distances (the opaque terrain pass is 58% to 92% of the GPU frame at render distance 32), so lowering render distance or resolution scale is still the biggest lever. Benchmark numbers: [Performance](08-performance.md).
