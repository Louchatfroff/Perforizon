# Perforizon vs Horizon: benchmarks

Horizon is upstream `horizon-rust/horizon` main at the commit this release was merged with, built from source with the
same `fast` profile and run with default settings; Perforizon is the merged tree, built the same way, with default
settings (Optimization Mode: Performance). "Auto-tuned" adds what the precise auto-tune chooses on this machine for the four
scenes: `farLodCoarse` with the handoff at 10 chunks (`farTerrainDistance 10`), which changes pixels (flatter distant
trees) and is off in 100% parity. Same machine, same seed, runs interleaved one after the other, never together.

- **Machine:** Intel 14 cores / 20 threads, NVIDIA GeForce RTX 4060 Ti, Windows 10, NVMe, 8 GB RAM, Vulkan.
- **Scenario:** `--benchmark standard` (seed 1234, four scenes: village, forest, ocean, mountain), offscreen 1280x720,
  vsync off, no antialiasing, simulation distance 12, render distance 16 and 32.
- **Method:** 3 full runs per build and distance, averaged. Run-to-run spread is about 1 to 3%. Raw files: `bench-out/U-*.json`, `H-*.json` and `M-*.json` (not part of the repository).
- **Reproduce:** `horizon-client --offscreen --benchmark standard --render-distance 32` writes a report to `<game dir>/benchmarks/`.

## Render distance 32

| Scene | Avg fps Horizon | Avg fps Perforizon | | Avg fps auto-tuned | | 1% low Horizon | 1% low Perforizon | | 1% low auto-tuned | | 0.1% low Horizon | 0.1% low Perforizon | | 0.1% low auto-tuned | |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| village | 1214 | 1278 | +5% | 1059 | -13% | 350 | 406 | +16% | 287 | -18% | 240 | 273 | +14% | 149 | -38% |
| forest | 943 | 1362 | +44% | 1950 | +107% | 349 | 471 | +35% | 463 | +33% | 232 | 312 | +34% | 205 | -12% |
| ocean | 1121 | 1410 | +26% | 2323 | +107% | 380 | 480 | +26% | 573 | +51% | 245 | 317 | +29% | 243 | -1% |
| mountain | 860 | 914 | +6% | 1088 | +26% | 198 | 245 | +24% | 284 | +44% | 125 | 182 | +46% | 145 | +16% |

## Render distance 16

| Scene | Avg fps Horizon | Avg fps Perforizon | | Avg fps auto-tuned | | 1% low Horizon | 1% low Perforizon | | 1% low auto-tuned | | 0.1% low Horizon | 0.1% low Perforizon | | 0.1% low auto-tuned | |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| village | 2166 | 2223 | +3% | 1947 | -10% | 731 | 768 | +5% | 549 | -25% | 454 | 440 | -3% | 327 | -28% |
| forest | 1989 | 2670 | +34% | 2496 | +25% | 701 | 859 | +23% | 662 | -6% | 415 | 502 | +21% | 378 | -9% |
| ocean | 2435 | 2941 | +21% | 2642 | +8% | 798 | 987 | +24% | 764 | -4% | 449 | 574 | +28% | 434 | -3% |
| mountain | 2131 | 2243 | +5% | 2054 | -4% | 565 | 723 | +28% | 575 | +2% | 301 | 449 | +49% | 330 | +10% |

## Reading the numbers

- With defaults, Perforizon beats Horizon in average fps in every scene at both distances (+3% to +44%) and in the 1% lows too (+5% to +35%, mountain +24% to +28%); the 0.1% lows are better in all scenes but village at render distance 16 (-3%).
- Gains are largest where there is a lot of geometry (forest: leaves; ocean: water) and at render distance 32, where the opaque terrain pass dominates. Village and mountain are cheap scenes limited by fixed per-frame work, so the gain there is small.
- Far LOD (the auto-tuned column) is not a free win: at render distance 32 it adds +107% in forest and ocean and +26% in mountain, but costs 13% in village; at render distance 16 it is level or slower. That is why it is off by default and the auto-tune measures all four scenes and keeps it only if the combined score improves. Run the precise auto-tune on your world and render distance.
- Optimization Mode parity removes the pixel-changing optimizations and their gains; the tables use Performance mode.
- One machine, one GPU, offscreen 720p. Treat the figures as a comparison between the builds, not as a promise for other hardware. The 0.1% lows are noisy.
