# Perforizon: technical details

Perforizon is a fork of Horizon (Rust, wgpu 30) aimed at higher frame rates and lower input latency. World
generation is bit-identical to Horizon. Optimizations do not have to reproduce every pixel, but game logic is
unchanged; the **Optimization Mode** setting (`optimizationMode:performance|parity`, `Graphics::exact_parity()`)
turns off every optimization that changes pixels.

All numbers below were measured on an RTX 4060 Ti, Vulkan, 1080p, unless stated. The per-pass GPU timer is
`HORIZON_GPU_PASSES=1` (`gfx/gpu_passes.rs`, `renderer.gpu_passes` in `horizon-ctl status`); CPU spans are
`HORIZON_SPIKES=MS` and `HORIZON_SPANS_FINE=1`.

## Terrain geometry

- **20-byte section vertex** (was 24): u16 fixed-point position and a `mode` field, shared with LOD. 16.7% less vertex memory.
- **Greedy meshing** (`mesher/greedy.rs`): merges coplanar faces with a tiling flag (bit 15 of `mode`) so textures
  repeat per block. About 5% fewer vertices on land; translucent faces go through the same merge, which makes the
  water pass about 80% cheaper in the ocean scene. Runtime switch `greedyMeshing`; automatically off with dynamic lights or shader packs.
- **Distant leaves** (`leafDistance`, default 5 chunks): beyond the band, sections mesh with solid leaves. About a
  third fewer terrain triangles in the forest scene at render distance 32. Hysteresis of two chunks and a cap of
  8 re-dirtied columns per frame avoid remesh storms; a mesh built under a stale near/far flag is re-queued.
- **Earlier LOD handoff** (`farTerrainDistance`, default 0): at Full far detail the tiles are not cheaper than
  occlusion-culled sections, so alone it is opt-in and left to the auto-tune.
- **Coarse far tiles** (`farLodCoarse`, default off): the tile quadtree splits at half the distance, so cells are 2x2 merged
  (about half the quads and triangles, trees reduced to blobs). With the handoff at 10 chunks, rd32, 1080p, same binary:
  avg fps village 1306 to 1751, forest 1262 to 1903, ocean 1225 to 1918, mountain 924 to 1421; terrain vertex invocations
  2.14/3.57/2.72/3.30 M to 1.19/1.66/1.22/2.12 M; GPU frame 0.50/0.73/0.57/0.66 ms to 0.33/0.44/0.35/0.48 ms. Handoff
  screenshots from altitude show no visible seam. Auto-tune step `farLod` (off, 16c, 10c).
- **Slimmer terrain vertex shader**: per-pixel fog and fade; about 6% off the terrain pass.

## Culling and frame setup

- Occlusion test boxes are the section's rounded-out vertex bounds instead of a fixed 18-block cube (`HORIZON_CULL_TIGHT=0` restores the cube): 3 to 5% fewer phase-1 triangles, box pass 25 to 35% cheaper.
- Sky, sun, moon and stars are drawn after phase 1, only where the depth buffer still holds the far value
  (`HORIZON_LATE_SKY=0` restores the old order). Exact parity, shader packs, the End and the title screen keep the old order.
- The per-section origins loop reads a cached upload time instead of a hash lookup: 0.118 to about 0.03 ms.
- Re-entry occlusion probe every 16th frame instead of every 64th.
- **Hi-Z rejection** (`hizOcclusion`, off by default, auto-tune candidate; `gfx/hiz.rs`, `hiz.wgsl`): the opaque depth (before translucent terrain) is kept at half resolution, reprojected next frame to the new camera into a farthest-depth pyramid, and a compute pass zeroes the instance counts of phase 1 draws whose box lies behind it. Rejected sections go on a rescue list tested against phase 1's depth, with copies of their draws in phase 2, so the picture is the culling one. Result at render distance 32 (RTX 4060 Ti, 1080p): phase 1 triangles -0.1% to -1.6%, GPU frame +0.07 ms, about 10% fewer fps: the last readback already leaves little hidden in phase 1. Vulkan classic draws only.

## Antialiasing

FXAA, TAA, SMAA, CMAA2 and FSR (sharpen/upscale) live in `gfx/aa/`. Cost at 1080p, render distance 32: FXAA 0.05 to 0.06 ms,
TAA 0.16 to 0.18, SMAA 0.15 to 0.19, CMAA2 0.13 to 0.20.

## CPU spikes

- Worker threads run below normal priority on Windows (`horizon-threads`), so meshing and streaming do not steal the render thread.
- Chunk freeing moved to a long-lived thread; mesher allocation fixes.
- Spans over `update_world`, `streaming`, `entity_geometry` and `encode` identify the remaining spikes. Span totals: `update_world` 734 to 203 ms, `renderer` 2181 to 856 ms over the same run.

## Mesh shaders

Implemented (`HORIZON_MESH_SHADERS`, `meshShaders`) but 2 to 4 times slower than the classic path on the RTX 4060 Ti
(forest 220 fps vs 998), so off by default. The auto-tune tests them and keeps them only if faster.

## Auto-tune

`crates/horizon-client/src/benchmark.rs`. Measures fps and 1% lows while the camera moves through the world, tries
candidate values (`graphics KEY VALUE`), waits for remeshing to settle after mesh-affecting options, keeps a
candidate only if it wins by more than 3%, and restores the player's position and state. Parity mode skips every
pixel-changing option and never lowers the resolution scale. See [perforizon-autotune.md](perforizon-autotune.md).

## PvP module

`crates/horizon-client-render/src/pvp.rs` (settings in `horizon-pvp.txt`, `horizon-ctl pvp KEY [VALUE]`, options page
Horizon > PvP) and `crates/horizon-client/src/game/pvp.rs`.

- **Latency**: hit prediction (swing and sound before the server confirms), crystal and anchor optimizers (hide the
  block or crystal at once, restore on rejection), elytra stop. Defaults on. Untested against a real server so far.
- **Visuals/HUD**: hitboxes (colors, rainbow, behind walls, eye line), projectile trails, damage flash, crosshair
  target color, armor/effect/coordinates/direction/ping/combo/reach/item-counter panels, TNT/crystal countdown, low fire, low shield,
  small totem, particle filters, toggle sprint and sneak with a status line.

## Build

Release profile (`tools/package_windows.ps1`): `lto = "fat"`, `codegen-units = 1`, unwinding kept. The `fast` profile
stays without LTO for quick iteration. Measured on the standard benchmark (RTX 4060 Ti, 1080p, median of 3
interleaved runs; outliers from background load are visible in single runs, so medians are used):

| rd | build | village | forest | ocean | mountain |
|----|-------|---------|--------|-------|----------|
| 32 | thin LTO (old) | 1334 | 1530 | 1391 | 973 |
| 32 | fat LTO | 1357 (first run; the others hit background load, 1042 and 509) | 1484 | 1416 | 1003 |
| 32 | fat + panic=abort | 1372 | 1542 | 1425 | 1011 |
| 16 | thin LTO | 2579 | 3048 | 3449 | 2546 |
| 16 | fat LTO | 2646 | 3016 | 3511 | 2555 |
| 16 | fat + panic=abort | 2686 | 3046 | 3608 | 2617 |

Fat LTO is worth 0 to 3% (build time 8m22 to 12m17); it costs the player nothing, so the release profile uses it.
`panic = "abort"` is worth at most about 2.5% at rd32 and is not used: shader pack compilation relies on
`crash::contain` (`catch_unwind`) to survive a driver-compiler panic, the save thread and pack loader re-raise worker
panics with `resume_unwind`, and two unit tests catch panics, all of which need unwinding.

mimalloc is already the global allocator of the client (`horizon_threads::Allocator`), so there was nothing to switch.

PGO was not run: `llvm-profdata` and `cargo-pgo` are not installed and the `llvm-tools` rustup component needs a
download. Steps when wanted: `rustup component add llvm-tools-preview`; `cargo install cargo-pgo`;
`cargo pgo build -- -p horizon-client --bin horizon-client`; run `--offscreen --benchmark standard` at rd16 and rd32
with the instrumented binary; `cargo pgo optimize build`. Expect a few percent on CPU-bound spans only, as the opaque
terrain pass is GPU-bound.

## Distribution

The Horizon launcher is adapted to install signed releases of `Louchatfroff/Perforizon`
(`launcher/src/backend.rs`); `tools/package_windows.ps1` and `packaging/windows/install-perforizon.ps1` build and
install the Windows zip. See [perforizon-install.md](perforizon-install.md).

## Network

TCP_NODELAY is set on every game connection (`Connection::from_stream`: client, LAN host, relay and friends tunnels), as vanilla does, and the client already reads on its own thread (`horizon-net`) and the LAN host writes on one thread per player. Writes on the client still happen on the game thread. The option `netLowLatency` (off by default, never toggled by the auto-tune) makes `ConnWriter` hold small packets and write them in one call: the client flushes at the start of each frame and after the tick-end packet, the LAN host's writer thread after each message, and a batch past 16 KiB is written at once. Memory connections (integrated server) never batch. Measured on loopback (`cargo test -p horizon-protocol --lib connection -- --nocapture`), a block-break or attack burst of three packets plus the answer takes about 15 to 17 microseconds through a socket either way: batching saves system calls and TCP segments, not round-trip time, and loopback Nagle costs little on Windows. No prediction change: vanilla's client-side movement prediction is already there and no rubber-banding gap was found.

## Latency counter

`latencyHud` (off by default) stamps the oldest input event (mouse motion, key, button) not yet shown, and the renderer
times it at `queue.present` (`Present`) and when the GPU reports the submission done (`Input`; polled at the next submit, so
up to one frame late). Rolling window of 240 frames, `crates/horizon-client-render/src/latency.rs`. It does not include the
display's scanout. Present modes and `frameLatency` are auto-tune candidates, see perforizon-autotune.md.

## SIMD and the pipeline cache

SIMD (AVX2) for `Cull::pack_box`, the section origins and light propagation was measured and not built. `pack_box` costs 6.4 ns per call (`cargo test --profile fast`, 8192 boxes), about 0.03 ms per frame for the sections in view; the whole `fc_solid_loop` span is 0.075 ms and the section origins (`fc_origins`) 0.02 ms of a CPU frame of about 0.5 ms, while the GPU needs about 0.8 ms. The most a vector path could save is under 0.02 ms per frame, below the auto-tune's 3% bar and invisible next to the GPU, and light propagation runs on the worker threads, outside the frame. No option was added.

The option `pipelineCache` (off by default, applies at the next start, Vulkan only) creates one `wgpu::PipelineCache`, loads `wgpu_pipeline_cache_vulkan_<vendor>_<device>.bin` from the game folder, hands it to every render and compute pipeline the game builds on its device, and writes it back (temporary file, then rename) after startup and at exit. A file over 256 MiB is ignored; data the driver rejects gives an empty cache. Measured with `bench-out/pcache.sh` (offscreen, three runs per mode in one game folder): pipelines ready in 118, 103, 104 ms with the option off and 132 (empty cache), 107, 106 ms with it on, also with the driver's own disk cache switched off (`__GL_SHADER_DISK_CACHE=0`: 118, 105 ms off; 148, 106, 106 ms on). The file is 1.2 MB. On this machine the cache saves nothing measurable because pipeline creation is about 105 ms in total; it exists for drivers without their own cache. The auto-tune cannot measure it per run and does not toggle it.

## Known gaps

Compact 16-byte vertex (under 3% gain) and persistent draw lists (CPU under 0.8 ms) were not done;
late camera sampling was not added: winit hands input over only between event-loop callbacks, so nothing new can be read
between `apply_mouse_look` (right after the frame's sleep and ticks) and the submit, and the frame limiter sleeps at the
end of a frame, before the next input is read;
PvP prediction is untested on a live server; the auto-tune button was verified through the same code path as `horizon-ctl autotune`, not clicked in the UI.

- **Shadow GPU culling** (`shadowIndirect`, off by default, auto-tune candidate only while a shader pack draws shadows; `gfx/shadow_cull.rs`, `shadow_cull.wgsl`): the shadow camera's section list is built with a column-level light test only, and a compute pass writes 0 into the instance count of every solid shadow draw whose section box is outside the light box. Same sections drawn as with the CPU test, so the picture is unchanged. Texel snapping of the shadow camera already exists (`native_shadow::camera`, `SNAP` cells), so there is no new shimmer work. Result (sample pack, render distance 16, 1080p, RTX 4060 Ti): shadow pass 0.295 ms to 0.280 ms. Needs the Vulkan/Metal multi-draw path and no flat terrain.

## Entities and particles

Measured at render distance 16, 1080p, RTX 4060 Ti, 200 NoAI cows in view (`bench-out/ep.sh`, `epsum.py`): CPU per frame 0.45 ms to 1.08 ms (mob geometry 0.35, vertex upload 0.22, material batching 0.09), GPU entity pass 0.08 ms. About 16000 smoke particles in view: CPU 0.44 to 1.43 ms (particle quads 0.39, upload 0.30, batch 0.12, tick 0.14), GPU particle pass 0.32 to 0.49 ms (fill-rate bound).

- **GPU particles** (`gpuParticles`, off by default, auto-tune candidate only with 2000 or more live particles, never in Exact parity; `gfx/gpu_particles.rs`, `vs_particle` in `entity.wgsl`): plain camera-facing quads go to the GPU as one 48-byte instance (center, size, uv, roll, color, light, layer) and are expanded in the vertex shader from the camera's right and up axes; shriek and vibration particles keep their two-quad CPU path. Drawn right after the opaque entities (opaque layer) and after translucent terrain (translucent layer), with the HDR mark pass. Used only on the classic path: a shader pack, a native mod or improved transparency falls back to CPU vertices (`Renderer::particle_instancing`). Same render, 16000 particles, same binary: CPU 1.43 to 0.95 ms per frame (vertex upload 0.30 to 0.05 ms, particle quads 0.38 to 0.27, batching 0.12 to 0.09), GPU unchanged. A screenshot with both modes shows identical pixels in the scene area; a unit test compares the expanded instances to the CPU vertices.
- **Entity instancing / GPU skinning**: not built. Mob quads are posed on the CPU into the one entity vertex stream that the shadow, outline, shader pack, improved transparency and glint passes also read, so skinning on the GPU would need a second path with fallbacks for all of them. The cost is 1.7 us per cow plus upload, only visible with hundreds of mobs. Particle simulation in compute was not attempted either: `ParticleEngine::tick` has per-type behaviour and world collision for about 100 particle types.

## Optimization wave 2 (all opt-in, decided by the auto-tune)

Every option below is off by default and is turned on only when the auto-tune measures it faster on the player's machine (by more than 3%, or by more than the measured noise in the precise auto-tune). Measured on an RTX 4060 Ti, Vulkan, 1080p offscreen; "no gain" means the auto-tune will keep it off here.

- **Far LOD** (`farLodCoarse`, with `farTerrainDistance`; the auto-tune step `farLod` tries off, 16c, 10c): the far terrain tile tree splits at half the distance, so cells are 2x2 merged, about half the triangles and no tree detail; it reuses the "reduced" tile plan of `farDetail:reduced`. Pixel-changing, so never in Exact parity. Gain at render distance 32 after the merge with upstream, handoff at 10 chunks: forest 1362 to 1950, ocean 1410 to 2323, mountain 914 to 1088, village 1278 to 1059 (-13%); at render distance 16 it is level or slower. Distant trees become flatter blobs; no visible seam in the screenshots taken. This is the one option of this wave that the auto-tune keeps on this machine.
- **Vertex pool** (`vertexPool`): the terrain vertex buffer becomes one page of up to 2 GiB; draw runs fall from about 220 to 3. Same pixels. Rd32 village 1204 to 1535 average fps and forest 1238 to 1272, but the runs are noisy (a first pass gave 1227 to 1456 and 1238 to 1260), so the real gain is a few percent on a GPU-bound frame. Turning it off later does not split the page again until the next world load.
- **Binary greedy** (`binaryGreedy`, needs `greedyMeshing`, pixel-changing only in the sense that it sits with the lossy steps): a sort-free merge pass on packed 64-bit keys instead of sorting 32-byte quads. The merge step is 1.2x to 2.4x faster; the bit-op face visibility part of the idea was not built. Whole-section meshing time was not measured, and meshing runs on worker threads, so expect the auto-tune to rarely keep it.
- **Hi-Z occlusion** (`hizOcclusion`): last frame's opaque depth, reprojected into a farthest-depth pyramid, rejects hidden sections before phase 1; rejected sections are re-tested and drawn in phase 2 where visible, so no holes. No gain here: 0.1 to 1.6% fewer triangles, the passes cost about 0.07 ms, net about 10% fewer fps. The existing readback culling already removes nearly everything hidden.
- **Texture compression** (`textureCompression`): BC7 mode-6 atlas, encoded on all cores per layer and per mip. 28.2 MB to 7.1 MB for 1291 layers of 64 px. No fps gain at rd16 and 1% lows 5 to 10% worse (animated sprites are written on the CPU), so it stays off here. Pixel-changing; not with a shader pack.
- **Shadow GPU culling** (`shadowIndirect`): see Known gaps; about 2% fps, within noise.
- **GPU particles** (`gpuParticles`): see Entities and particles. CPU only; no GPU gain.
- **Latency** (`presentMode` mailbox, `frameLatency` 2, `latencyHud`): measurable only in a real window; the tie rule favours the lower latency value.
- **Network** (`netLowLatency`), **pipeline cache** (`pipelineCache`): no measurable gain here; never toggled by the auto-tune.
- **Chunk storage** and **SIMD**: not built; blocks are already paletted and shared when uniform, and the loops SIMD would speed up cost under 0.1 ms per frame.
- **Build**: the release profile uses fat LTO (0 to 3%, close to noise); `panic = "abort"` was rejected because `crash::contain` relies on `catch_unwind`.

## Precise auto-tune

`crates/horizon-client/src/benchmark/precise.rs`, started by Advanced Graphics > Auto-tune > Run Precise Auto-tune... or `horizon-ctl autotune --precise [FPS [PASSES]]` (`autotune --cancel` stops it). It runs the standard benchmark's four scenes (village, forest, ocean, mountain; seed 1234, creative), which only exist in the benchmark world, so it saves and leaves the player's world, replaces the `horizon-benchmark` world, runs there and ends at the title screen. Each configuration is measured as PASSES (default 3) flights per scene, each flight half of the standard path at the standard speed (6 s). Score per flight: 0.4 x average fps + 0.4 x 1% low + 0.2 x 0.1% low; per pass the geometric mean over the four scenes; per configuration the mean over passes. The noise estimate is the relative range (max - min over mean) of the pass scores, taken from the baseline and from the incumbent, with a floor of 2%. Coordinate descent over the same candidate list as the quick auto-tune (defaults first), keeping a candidate only if its score exceeds the incumbent's by more than the noise; ties go to the earlier value. A final validation measures all defaults and the chosen set again and reverts to the defaults unless the chosen set scores higher. A 90 minute budget stops the search early. Pure scoring, margin and ordering logic has unit tests in `precise.rs`.
