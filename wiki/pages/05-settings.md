---
title: Settings
section: Use
summary: Every Perforizon graphics option with its default and effect, and the horizon-ctl commands.
---

# Settings

All of these are in `Options > Video Settings > Advanced Graphics`. They are stored in `horizon-graphics.txt` in the game folder. Most are opt-in: the [auto-tune](04-auto-tune.md) turns them on only where they measure faster.

## Set an option from the command line

With the game running:

```sh
horizon-ctl graphics KEY VALUE     # set an option now
horizon-ctl graphics-save          # write the options to disk
horizon-ctl graphics-dump          # print the current options
horizon-ctl status                 # current options and the auto-tune result
```

Example: `horizon-ctl graphics meshShaders true`. `horizon-ctl perf-mode on` measures without a frame cap, VSync or the idle limit; `off` restores them. It is a benchmarking helper, not the Optimization Mode.

## Mode

| Option | Default | Notes |
| --- | --- | --- |
| `optimizationMode` | `performance` | `parity` disables the pixel-changing optimizations below. See [Parity mode](09-parity-mode.md). |

## Geometry and terrain

| Option | Default | Effect |
| --- | --- | --- |
| `greedyMeshing` | on | Merges coplanar faces, water included. About 5% fewer vertices, up to 80% cheaper water. Off in parity mode and with dynamic lights or shader packs. |
| `binaryGreedy` | off | Needs `greedyMeshing`. Sort-free merge of the quads; the merge step is 1.2x to 2.4x faster, same quads. Whole meshing time and fps were not measured. |
| `leafDistance` | 5 | Chunks within which leaves keep the fancy cut-out look; beyond it they are solid. `0` keeps fancy everywhere. About a third fewer triangles in forests at render distance 32. |
| `farTerrainDistance` | 0 (off) | Hands sections to the distant-terrain tiles earlier. Alone it is a small gain or a loss; it pays with `farLodCoarse`. |
| `farLodCoarse` | false | Far terrain tiles use twice-as-large cells from half the usual distance: a quarter of the quads, no tree detail. Changes pixels. Only has an effect with `farTerrainDistance` (or `lodDistance`) on. |
| `vertexPool` | off | Same pixels. One terrain page of up to 2 GiB instead of several 256 MiB ones; draw runs 220 to 3 at render distance 32. |
| `textureCompression` | off | BC7, lossy. Atlas 28.2 MB to 7.1 MB. Skipped in parity mode and with a shader pack. Applies through a resource reload. |

## Culling and effects

| Option | Default | Effect |
| --- | --- | --- |
| `caveCulling`, `entityCulling`, `shadowCache` | on | Lossless or near-lossless. |
| `meshShaders` | off | Faster on some GPUs; 2 to 4 times slower on the RTX 4060 Ti measured. Let the auto-tune decide. |
| `hizOcclusion` | off | Same pixels. Hi-Z rejection of hidden sections; measured about 10% slower on the RTX 4060 Ti. |
| `shadowIndirect` | off | Same pixels. GPU culling of shadow draws; only relevant while a shader pack draws a shadow map. About 2% fps, within noise. |
| `gpuParticles` | off | Builds particle quads on the GPU. Slightly pixel-changing (draw order). About 0.5 ms less CPU per frame at about 16000 particles. Performance mode only. |

## Latency and network

| Option | Default | Effect |
| --- | --- | --- |
| `presentMode` | `auto` | `auto`, `fifo`, `mailbox` or `immediate`. Only used with VSync off. |
| `frameLatency` | 0 (auto: 1, or 2 on Fifo) | Frames queued ahead, 0 to 3. |
| `latencyHud` | off | Shows the input-to-screen delay. See [Latency](06-latency.md). |
| `netLowLatency` | off | Batches small network packets. Same bytes on the wire. Never toggled by the auto-tune. |
| `pipelineCache` | off | Keeps the driver's compiled pipelines on disk (Vulkan only). Applies at the next start. |

## Other related options

| Option | Default | Effect |
| --- | --- | --- |
| `resolutionScale` | 100 | 50 to 200. The auto-tune may lower it in Performance mode to reach a target. |
| Antialiasing | none | FXAA about 0.05 ms, TAA 0.16 to 0.18 ms, SMAA 0.15 to 0.19 ms, CMAA2 0.13 to 0.20 ms at 1080p. FSR is available too. |

See [Troubleshooting](13-troubleshooting.md) if an option misbehaves, and [Performance](08-performance.md) for what each costs and gains.
