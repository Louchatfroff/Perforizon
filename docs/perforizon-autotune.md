# Auto-tune: installing Perforizon and getting the most frames

Perforizon is a performance fork of Horizon. Its defaults are already faster than Horizon; the in-game
auto-tune measures your machine and keeps the settings that run best on it.

## 1. Install

Windows x64, PowerShell, no admin rights:

```powershell
irm https://raw.githubusercontent.com/Louchatfroff/Perforizon/main/packaging/windows/install-perforizon.ps1 | iex
```

This installs to `%LOCALAPPDATA%\Perforizon`. Or use the adapted launcher (see [perforizon-install.md](perforizon-install.md)),
which installs and updates Perforizon releases. Either way you then prepare the game files from your own copy
of Minecraft Java 26.3, as for Horizon (`PLAY.txt` in the install folder).

## 2. Pick an Optimization Mode

`Options > Video Settings > Advanced Graphics > Auto-tune > Optimization Mode`:

| Mode | What it does |
| --- | --- |
| **Performance** (default) | Allows every optimization that changes a few pixels: greedy meshing, solid distant leaves (`leafDistance`), merged water faces, and the auto-tune can lower the resolution scale. Game logic and world generation are untouched. |
| **100% parity** | Turns off every optimization that changes pixels. Frames match Horizon's picture; only lossless speedups stay. Auto-tune never lowers the resolution scale and never tries the pixel-changing options. |

The same switch from the command line: `horizon-ctl perf-mode` is a frame-cap helper for benchmarks; the mode itself is
`horizon-ctl graphics optimizationMode performance|parity`.

## 3. Run the auto-tune

1. Open a **creative** world (or your integrated single-player world) and stand where you normally play, with
   your usual render distance. The tune runs in the world you are in, so loaded terrain is what gets measured.
2. `Options > Video Settings > Advanced Graphics > Auto-tune > Run Auto-tune...`
3. The options screen closes, the camera is moved for a short measurement, and the game restores your position
   and state when it is done. It takes up to about two minutes (hard cap 110 s of measuring, plus the remesh
   waits) and ends with a chat notice: `Auto-tune: done in Ns, X fps (1% low Y); kept key=value, ...`.
4. The result is saved to your graphics options, so it persists across restarts.

Command line equivalent (the game must be running a world; `autotune --precise` is described in 4b):

```sh
horizon-ctl autotune 600        # target fps; below it the resolution scale may be lowered (Performance mode only)
horizon-ctl status              # shows the result and the current options
```

What it tries, in order, keeping a candidate only if it beats the current choice by more than 3%:
mesh shaders (off unless faster), cave culling, entity culling, shadow cache, greedy meshing, then in Performance
mode `leafDistance` (5, 3, 0) and `farTerrainDistance` and `farLodCoarse` together as one step (off, 16 chunks coarse, 10 chunks coarse; tried first), and finally the resolution scale if you
asked for a target your machine does not reach. After each change that remeshes terrain it waits for the
chunks to settle before measuring.

## 4. Tuning by hand

All of these are in Advanced Graphics, and `horizon-ctl graphics KEY VALUE` sets them live (`graphics-save` writes them).

| Option | Default | Notes |
| --- | --- | --- |
| `optimizationMode` | `performance` | `parity` disables the pixel-changing optimizations below. |
| `greedyMeshing` | on | Merges coplanar faces, including water. About 5% fewer vertices, up to 80% cheaper water. Off in parity mode and with dynamic lights or shader packs. |
| `binaryGreedy` | off | Needs `greedyMeshing`. A sort-free merge of the quads (packed 64-bit keys, bit rows) instead of sorting 32-byte structs: the merge step is 1.2x to 2.4x faster, same quads. Whole meshing time and fps not measured; tried right after `greedyMeshing` in Performance mode. |
| `leafDistance` | 5 | Chunks within which leaves keep the fancy cut-out look; beyond it they are solid. `0` keeps fancy everywhere. About a third fewer triangles in forests at render distance 32. |
| `farTerrainDistance` | 0 (off) | Hands sections over to the distant-terrain tiles earlier. Alone it is a small gain or a loss; it pays with `farLodCoarse`. |
| `farLodCoarse` | false | Far terrain tiles use twice-as-large cells (a quarter of the quads, no tree detail) from half the usual distance. Changes pixels, so never tried in Exact parity. Only has an effect with `farTerrainDistance` (or `lodDistance`) on. |
| `caveCulling`, `entityCulling`, `shadowCache` | on | Lossless or near-lossless. |
| `meshShaders` | off | Faster on some GPUs, 2 to 4 times slower on the RTX 4060 Ti we measured. Let the auto-tune decide. |
| `vertexPool` | off | Same pixels. Keeps the terrain in one page of up to 2 GiB instead of several 256 MiB ones (draw runs 220 to 3 at render distance 32); existing pages are emptied into it over a few frames. Turning it off later does not split the page again until the next world load. Tried first by the auto-tune, in both modes. |
| `textureCompression` | off | Changes pixels (BC7, lossy; skipped in Exact parity and with a shader pack, never tried there). Packs the block sprites as BC7 blocks where the GPU has them: 28.2 MB to 7.1 MB for 1291 layers of 64 px, same blocks per texel fetch, mips cut per sprite so nothing bleeds between tiles. Animated sprites are then written on the CPU (frames cached), which costs a little at frame changes. Applies through a resource reload (about 2 s); the auto-tune waits for it, and keeps it only when it beats off by more than 3%. On the reference machine (RTX 4060 Ti, rd16) it did not: average fps equal within noise, 1% lows 5 to 10% lower, so the auto-tune leaves it off there. |
| `hizOcclusion` | off | Same pixels. Rejects phase 1 sections hidden behind last frame's depth (reprojected to the new camera) before they are drawn; a rejected section whose box is still seen is drawn in phase 2, so it cannot leave holes. Tried in both modes; measured 0 to 1.6% fewer triangles and a loss of about 10% fps on the RTX 4060 Ti, so it stays off unless a machine measures otherwise. |
| `gpuParticles` | off | Pixel-changing slightly (draw order of particle quads), so tried in Optimization Mode Performance only, and only when the world holds at least 2000 live particles at tune start (`live_particles` in `horizon-ctl status`). Builds particle quads on the GPU from one 48-byte record each instead of four 44-byte vertices. About 0.5 ms less CPU per frame at about 16000 particles; no GPU gain. |
| `shadowIndirect` | off | Same pixels. Only tried while a shader pack draws a shadow map (the status reports `shadow_pass`). The CPU keeps the light box's column test and skips the per-section one; a compute pass (`gfx/shadow_cull.rs`) zeroes the instance count of each solid shadow draw whose section box lies outside the light box, the same test and box as the CPU's. Classic multi-draw only. Measured with the sample pack at render distance 16 (RTX 4060 Ti): shadow pass 0.295 to 0.280 ms, about 2% fps, within noise, so it stays off unless a machine measures more than 3%. |
| `netLowLatency` | off | Holds small network packets and sends them as one train, at the start of each frame and the end of each tick (client) and after each message (LAN host). Same bytes on the wire (protocol 777 unchanged). The auto-tune cannot measure a network and never toggles it. |
| `pipelineCache` | off | Keeps the driver's compiled pipelines in `wgpu_pipeline_cache_vulkan_<vendor>_<device>.bin` in the game folder (Vulkan only; read at start, written at exit). Applies at the next start, so the auto-tune never toggles it. On the RTX 4060 Ti all pipelines are ready in about 105 ms with it on or off (the NVIDIA driver keeps its own cache), so there is nothing to gain here; it is for drivers that leave caching to the application. |
| `presentMode` | auto | The auto-tune also tries `mailbox` (VSync is off while it runs). Only used with VSync off. |
| `frameLatency` | 0 (auto: 1, or 2 on Fifo) | The auto-tune also tries 2 queued frames. |
| `latencyHud` | off | Shows the input-to-screen delay (average and worst of the last 240 frames) in the PvP HUD corner of the ping counter, and under `input_latency_ms` in `horizon-ctl status`. Never tuned; it does not change a frame. |
| Antialiasing | none | FXAA about 0.05 ms, TAA 0.16 to 0.18 ms, SMAA 0.15 to 0.19 ms, CMAA2 0.13 to 0.20 ms at 1080p. |

### Latency steps

`presentMode` and `frameLatency` change no pixels, so they run in Exact parity too. The offscreen benchmarks have no
swapchain and cannot see them; the in-game auto-tune does (a real window, VSync off). For these two steps the pick is
not "beats the first value by 3%": every candidate within 3% of the best score is a tie, and the tie goes to the
first value in the list, which is the lower latency one (`auto` presents with Immediate, 1 frame of queue). A
higher-latency value is kept only when it is clearly faster.

## 4b. Precise auto-tune

The quick auto-tune measures 3 seconds per candidate in the world you stand in. The **precise** one is slower and
more careful: it measures the same four scenes as the stock-vs-Perforizon comparison (village, forest, ocean,
mountain, seed 1234), several passes per candidate.

- Start it with `Options > Video Settings > Advanced Graphics > Auto-tune > Run Precise Auto-tune...` or
  `horizon-ctl autotune --precise [FPS [PASSES]]` (PASSES defaults to 3). Click the button again, or run
  `horizon-ctl autotune --cancel`, to stop it; a cancelled or failed run restores your graphics options.
- The scenes exist only in the benchmark world (`saves/horizon-benchmark`, replaced on every run). So the precise
  run saves and leaves your world, opens the benchmark world, and ends at the title screen: reopen your world from
  the list. It works from the title screen too. Not available while connected to a remote server.
- Progress shows as chat notices (`Precise auto-tune: 5/15 hizOcclusion=true: -6.5% (needs 9.7%)`); the result is
  written to `autotune.json` (`mode: precise`, per-scene averages and lows of every try, pass scores, noise,
  validation, `kept`, `seconds`) and the chosen options are saved.
- Score of one flight: 0.4 x average fps + 0.4 x 1% low + 0.2 x 0.1% low (lows weigh more, the noisy 0.1% low less). One pass is the geometric
  mean of the four scenes' scores; a configuration's score is the mean over its passes. A flight is the first half of
  the standard benchmark path at the standard speed (6 s).
- Noise: the relative range (max - min over mean) of the pass scores of the baseline and of the current choice, never
  below 2%. A candidate is kept only if its score beats the current choice by more than that. Ties go to the earlier
  value in the list (the default, or for `presentMode`/`frameLatency` the lower latency one).
- Candidates are the same as the quick auto-tune (defaults first, one at a time, coordinate descent): `farLod`, `vertexPool`,
  `meshShaders`, `hizOcclusion`, `caveCulling`, `entityCulling`, `shadowCache`, `shadowIndirect` (only with a shader pack),
  `presentMode`, `frameLatency`, and in Performance mode `textureCompression` (if supported), `greedyMeshing`,
  `binaryGreedy`, `gpuParticles` (only if 2000 particles are live, which the benchmark world rarely has) and `leafDistance`.
  Exact parity mode skips the pixel-changing ones and never lowers the resolution scale.
- Then a **validation**: defaults and the chosen set are measured again; if the chosen set does not score higher, the
  defaults are restored. Only then, in Performance mode, the resolution scale is lowered if FPS is not reached.
- Time: about 100 s per candidate on the reference machine (4 scenes x chunk loads + 3 flights), so the 15 candidates
  take about 32 minutes. A 90 minute budget ends the search early.

Reference run (RTX 4060 Ti, Vulkan, 1920x1080 offscreen, render distance 32, Performance mode, 3 passes, started from a
creative world): 1930 s, kept `farLod=10c` only. Baseline score 587 (average fps village/forest/ocean/mountain
1258/1248/1400/978), after `farLod=10c` 864 (1919/1851/2253/1579); validation defaults 614, chosen 864, not reverted.
The noise margin was 8 to 10% from the second candidate on (baseline spread 2.7%; the 0.1% lows are noisy), so
`vertexPool` (+3.0%) and every other small effect were not kept; `meshShaders` -25%, `hizOcclusion` -6.5%,
`textureCompression` -3.7%, `binaryGreedy` -1.1%. Not run end to end here: Exact parity mode (unit tests cover the
candidate filter), cancel from the button.

## 5. What to expect

The GPU is the limit at high render distances (the opaque terrain pass is 58 to 92% of the GPU frame at render
distance 32), so lowering render distance or resolution scale is the biggest lever. Results vary with your GPU:
run the auto-tune once per machine and once after changing render distance. If you change shader packs, run it again.

Benchmark numbers against Horizon are in [perforizon-benchmarks.md](perforizon-benchmarks.md); how each
optimization works is in [perforizon-technical.md](perforizon-technical.md).
