---
title: Performance
section: Reference
summary: What makes Perforizon faster, the measured benchmark tables, and how to reproduce them.
---

# Performance

World generation and game logic are unchanged; the work is in rendering and in CPU spikes. Numbers below come from `docs/perforizon-benchmarks.md` and `docs/perforizon-technical.md` in the repository. The option names are explained in [Settings](05-settings.md).

## What makes it faster

**Geometry**
- A 20-byte terrain vertex instead of 24: 16.7% less vertex memory.
- Greedy meshing merges coplanar faces and goes through the same merge for water: about 5% fewer vertices on land and a water pass about 80% cheaper in the ocean scene. Off with dynamic lights or shader packs.
- Solid distant leaves (`leafDistance`, default 5 chunks): about a third fewer terrain triangles in forests at render distance 32.
- A slimmer terrain vertex shader: about 6% off the terrain pass.

**Culling and frame setup**
- Occlusion boxes are the section's real bounds instead of a fixed cube: 3% to 5% fewer phase-1 triangles, box pass 25% to 35% cheaper.
- Late sky: sun, moon, stars and sky are drawn only where the depth buffer still holds the far value. Parity mode, shader packs, the End and the title screen keep the old order.
- Cheaper per-frame setup (section origins read a cached upload time) and a more frequent re-entry occlusion probe.

**CPU spikes**
- Worker threads run below normal priority on Windows, so meshing and streaming do not steal the render thread.
- Chunk freeing runs on its own long-lived thread; mesher allocation fixes.

**Far LOD (opt-in)**
- `farLodCoarse` with `farTerrainDistance` makes distant tiles coarser (about half the triangles, no tree detail). It is the one optional feature that pays off on the reference machine, and it is not free: see below.

## Benchmark setup

- Machine: Intel 14 cores / 20 threads, NVIDIA GeForce RTX 4060 Ti, Windows 10, NVMe, 8 GB RAM, Vulkan.
- Scenario: `--benchmark standard` (seed 1234; village, forest, ocean, mountain), offscreen 1280x720, VSync off, no antialiasing, simulation distance 12, render distance 16 and 32.
- Horizon is upstream main at the commit this release was merged with, built from source with the same `fast` profile and default settings. Perforizon is the merged tree built the same way, Performance mode.
- 3 full runs per build and distance, averaged. Run-to-run spread is about 1% to 3%.
- "Auto-tuned" adds what the precise auto-tune chose on this machine: `farLodCoarse` with the handoff at 10 chunks. It changes pixels (flatter distant trees) and is off in 100% parity.

## Render distance 32: average fps

| Scene | Horizon | Perforizon | Change | Auto-tuned | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| village | 1214 | 1278 | +5% | 1059 | -13% |
| forest | 943 | 1362 | +44% | 1950 | +107% |
| ocean | 1121 | 1410 | +26% | 2323 | +107% |
| mountain | 860 | 914 | +6% | 1088 | +26% |

## Render distance 32: 1% lows

| Scene | Horizon | Perforizon | Change | Auto-tuned | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| village | 350 | 406 | +16% | 287 | -18% |
| forest | 349 | 471 | +35% | 463 | +33% |
| ocean | 380 | 480 | +26% | 573 | +51% |
| mountain | 198 | 245 | +24% | 284 | +44% |

## Render distance 16: average fps

| Scene | Horizon | Perforizon | Change | Auto-tuned | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| village | 2166 | 2223 | +3% | 1947 | -10% |
| forest | 1989 | 2670 | +34% | 2496 | +25% |
| ocean | 2435 | 2941 | +21% | 2642 | +8% |
| mountain | 2131 | 2243 | +5% | 2054 | -4% |

## Render distance 16: 1% lows

| Scene | Horizon | Perforizon | Change | Auto-tuned | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| village | 731 | 768 | +5% | 549 | -25% |
| forest | 701 | 859 | +23% | 662 | -6% |
| ocean | 798 | 987 | +24% | 764 | -4% |
| mountain | 565 | 723 | +28% | 575 | +2% |

The 0.1% lows are in `docs/perforizon-benchmarks.md`; they are noisy and better in all scenes but village at render distance 16 (-3%).

## Reading the numbers

- With defaults Perforizon beats Horizon in average fps in every scene at both distances (+3% to +44%).
- The gain is largest where there is a lot of geometry (forest leaves, ocean water) and at render distance 32, where the opaque terrain pass dominates. Village and mountain are cheap scenes limited by fixed per-frame work.
- Far LOD is not a free win: at render distance 32 it adds +107% in forest and ocean and +26% in mountain, but costs 13% in village, and at render distance 16 it is level or slower. That is why it is off by default and the precise auto-tune keeps it only if the combined score improves.
- 100% parity removes the pixel-changing optimizations and their gains; the tables use Performance mode. See [Parity mode](09-parity-mode.md).
- One machine, one GPU, offscreen 720p. Treat the figures as a comparison between two builds, not a promise for other hardware.

## Reproduce

```sh
horizon-client --offscreen --benchmark standard --render-distance 32
```

The report is written to `<game dir>/benchmarks/`. Run Horizon and Perforizon built with the same profile, on the same machine and seed, one after the other, never together, and average several runs. Tools for finding where time goes: `HORIZON_GPU_PASSES=1` (per-pass GPU timer, `renderer.gpu_passes` in `horizon-ctl status`) and `HORIZON_SPIKES=MS` for CPU spikes. Then run the [auto-tune](04-auto-tune.md) for your own hardware.

## Measured, not kept

Mesh shaders (2 to 4 times slower here), Hi-Z occlusion (about 10% slower), BC7 texture compression (no fps gain), vertex pool, shadow GPU culling and GPU particles (within noise) are implemented and off by default; other GPUs may differ, which is what the auto-tune is for. PGO and AVX2 paths were not built.
