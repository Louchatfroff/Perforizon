---
title: Parity mode
section: Reference
summary: Performance mode against 100% parity: what each one changes and what stays identical.
---

# Parity mode

The Optimization Mode setting has two values. Set it in `Options > Video Settings > Advanced Graphics > Auto-tune > Optimization Mode`, or with `horizon-ctl graphics optimizationMode performance|parity`.

## What never changes

Both modes leave world generation and game logic untouched. The same seed gives the same world, in either mode and compared with Horizon. The mode only decides whether rendering optimizations may change a few pixels.

## Performance (default)

Allows every optimization that changes pixels slightly:

| Feature | Visible effect |
| --- | --- |
| Greedy meshing, water included | Merged faces; textures repeat per block. |
| Solid distant leaves (`leafDistance`) | Leaves past 5 chunks are solid instead of cut-out. |
| Far LOD (`farLodCoarse`) | Distant trees become flatter blobs. Opt-in. |
| BC7 texture compression | Lossy atlas. Opt-in. |
| GPU particles | Different draw order of particle quads. Opt-in. |
| Resolution scale | The auto-tune may lower it to reach an fps target. |

## 100% parity

Turns off every optimization that changes pixels. Frames match Horizon's picture; only lossless speedups stay. In this mode:

- The auto-tune never lowers the resolution scale and never tries the pixel-changing options.
- Greedy meshing, `leafDistance` and `farLodCoarse` have no effect, and BC7 and GPU particles are skipped.
- The sky keeps its old draw order.
- The latency steps (`presentMode`, `frameLatency`) and the lossless options still run, because they change no pixels. See [Latency](06-latency.md).

Lossless options that stay available include cave culling, entity culling, the shadow cache, the vertex pool and Hi-Z occlusion (same pixels).

## Which to pick

- Compare screenshots, record footage that must match Horizon, or file a rendering bug: parity.
- Everything else: Performance, and run the [auto-tune](04-auto-tune.md).

The benchmark tables in [Performance](08-performance.md) use Performance mode; parity gives up their pixel-changing gains.

## Check it

`horizon-ctl status` shows the current options, including the mode. The `Graphics::exact_parity()` check in the source is what every pixel-changing feature consults. Option details: [Settings](05-settings.md).
