# Perforizon vs stock Horizon: benchmarks

Stock is the official Horizon nightly (SHA-256 verified); Perforizon is this tree built with the `fast` profile and
default settings (Optimization Mode: Performance). "Auto-tuned" adds what the precise auto-tune kept on this machine: `farLodCoarse` with the handoff at 10 chunks (`farTerrainDistance 10`), which changes pixels (flatter distant trees) and is off in 100% parity. Same machine, same seed, run one after the other, never together.

- **Machine:** Intel 14 cores / 20 threads, NVIDIA GeForce RTX 4060 Ti, Windows 10, NVMe, 8 GB RAM, Vulkan.
- **Scenario:** `--benchmark standard` (seed 1234, four scenes: village, forest, ocean, mountain), offscreen 1280x720,
  vsync off, no antialiasing, simulation distance 12, render distance 16 and 32.
- **Method:** 3 full runs per build and distance, averaged. Run-to-run spread is about 1 to 3%. Raw files: `bench-out/G-*.json` (not part of the repository).
- **Reproduce:** `horizon-client --offscreen --benchmark standard --render-distance 32` writes a report to `<game dir>/benchmarks/`.

## Render distance 32

| Scene | Avg fps stock | Avg fps Perforizon | | Avg fps auto-tuned | | 1% low stock | 1% low Perforizon | | 1% low auto-tuned | | 0.1% low stock | 0.1% low Perforizon | | 0.1% low auto-tuned | |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| village | 1047 | 1275 | +22% | 1763 | +68% | 292 | 390 | +33% | 444 | +52% | 183 | 259 | +42% | 220 | +20% |
| forest | 998 | 1480 | +48% | 2446 | +145% | 332 | 509 | +53% | 606 | +83% | 216 | 327 | +52% | 245 | +13% |
| ocean | 1148 | 1351 | +18% | 2753 | +140% | 360 | 488 | +35% | 732 | +103% | 234 | 330 | +41% | 275 | +18% |
| mountain | 763 | 950 | +24% | 1527 | +100% | 140 | 238 | +70% | 381 | +172% | 91 | 159 | +75% | 187 | +106% |

## Render distance 16

| Scene | Avg fps stock | Avg fps Perforizon | | Avg fps auto-tuned | | 1% low stock | 1% low Perforizon | | 1% low auto-tuned | | 0.1% low stock | 0.1% low Perforizon | | 0.1% low auto-tuned | |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| village | 1928 | 2442 | +27% | 2348 | +22% | 607 | 756 | +24% | 661 | +9% | 362 | 430 | +19% | 359 | -1% |
| forest | 2151 | 3018 | +40% | 3214 | +49% | 683 | 917 | +34% | 892 | +30% | 399 | 479 | +20% | 438 | +10% |
| ocean | 2571 | 3216 | +25% | 3186 | +24% | 828 | 1076 | +30% | 951 | +15% | 464 | 555 | +20% | 451 | -3% |
| mountain | 2182 | 2399 | +10% | 2388 | +9% | 519 | 747 | +44% | 694 | +34% | 262 | 407 | +55% | 375 | +43% |

## Reading the numbers

- With defaults, Perforizon beats stock Horizon in average fps in every scene at both distances (+10% to +48%), and in the 1% and 0.1% lows too, because it cuts CPU spikes and remesh work.
- Gains are largest where there is a lot of geometry (forest: leaves; ocean: water) and at render distance 32, where the opaque terrain pass dominates.
- The auto-tuned column is where Far LOD pays: +68% to +145% average fps at render distance 32. At render distance 16 it is level with the defaults or a few percent below (the distant tiles take over from sections that were cheap anyway), so the auto-tune decides per render distance.
- Optimization Mode parity removes the pixel-changing optimizations and their gains; the tables use Performance mode.
- One machine, one GPU, offscreen 720p. Treat the figures as a comparison between the builds, not as a promise for other hardware. The 0.1% lows of the auto-tuned build are noisy.
