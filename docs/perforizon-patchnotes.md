# Perforizon patch notes

## 0.1.0 (unreleased)

First Perforizon build, forked from Horizon (Minecraft Java 26.3 in Rust). World generation is bit-identical.

### Performance
- 20-byte terrain vertex (was 24): 16.7% less vertex memory.
- Greedy meshing, water included (about 80% cheaper water pass in the ocean scene).
- Solid distant leaves (`leafDistance`, default 5 chunks): about a third fewer terrain triangles in forests at render distance 32.
- Tighter occlusion boxes, late sky pass, slimmer terrain vertex shader (about -6% terrain pass), cheaper frame setup.
- CPU spikes: lower-priority worker threads, chunk freeing on its own thread, mesher allocation fixes.
- Optional: `farTerrainDistance` (earlier distant-terrain handoff, off), mesh shaders (off; slower on the GPU we measured).
- New antialiasing modes: SMAA, CMAA2, FSR, plus FXAA/TAA.
- Results against stock: [perforizon-benchmarks.md](perforizon-benchmarks.md).

### Settings
- Optimization Mode: Performance or 100% parity (Advanced Graphics > Auto-tune).
- In-game Auto-tune (`Run Auto-tune...` or `horizon-ctl autotune`): [perforizon-autotune.md](perforizon-autotune.md).
- `horizon-ctl`: `graphics`, `graphics-save`, `perf-mode`, `autotune`, `pvp`, and more fields in `status`.

### PvP
- Latency: hit prediction, crystal and anchor optimizers, elytra stop.
- Visuals: hitboxes, projectile trails, damage flash, crosshair target color, armor/effect/coordinates/direction/ping/combo/reach/item HUDs, explosion countdown, low fire, low shield, small totem, particle filters, toggle sprint/sneak. Options page Horizon > PvP.

### Distribution
- The launcher and the Windows installer target `Louchatfroff/Perforizon` signed releases ([perforizon-install.md](perforizon-install.md)).

### Known gaps
- PvP prediction is untested on a live server.
- Windows only is packaged.

### Optimization wave 2 (opt-in, the auto-tune decides)
All of these are off by default. On the RTX 4060 Ti test machine only Far LOD paid off; the others are correct but showed no gain or were within noise, so the auto-tune keeps them off there. Other GPUs may differ.
- Far LOD (`farLodCoarse`): coarser distant terrain tiles, +34 to +56% fps at render distance 32 with the handoff at 10 chunks. Changes pixels (flatter distant trees); not in 100% parity.
- Vertex pool (`vertexPool`): one big terrain buffer, draw runs 220 to 3; a few percent on this GPU.
- Binary greedy meshing (`binaryGreedy`): sort-free merge, 1.2x to 2.4x faster merge step; total meshing time not measured.
- Hi-Z occlusion (`hizOcclusion`): no gain here (about 10% slower), left off.
- BC7 texture compression (`textureCompression`): atlas 28.2 MB to 7.1 MB, no fps gain, 1% lows 5 to 10% worse here.
- Shadow GPU culling (`shadowIndirect`, shader packs), GPU particles (`gpuParticles`): within noise on the GPU, less CPU with many particles.
- Latency: `presentMode` mailbox and `frameLatency` candidates, input latency counter (`latencyHud`); batched network packets (`netLowLatency`); on-disk pipeline cache (`pipelineCache`).
- Release builds use fat LTO.
- Precise auto-tune (`Run Precise Auto-tune...`, `horizon-ctl autotune --precise`): the benchmark scenes, several passes per candidate, noise-aware keep rule, final validation against the defaults. Slow (tens of minutes); see [perforizon-autotune.md](perforizon-autotune.md).
- `horizon-ctl`: `autotune --precise|--cancel`, `close-world`, `graphics-dump`.
