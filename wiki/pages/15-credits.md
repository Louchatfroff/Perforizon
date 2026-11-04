---
title: Credits
section: Project
summary: Horizon and its contributors, the MIT licence, fonts, third-party notices and the disclaimer.
---

# Credits

## Horizon

Perforizon is a fork of [Horizon](https://github.com/horizon-rust/horizon), Minecraft: Java Edition 26.3 rewritten in Rust, by Ludovic Marie and the Horizon contributors. Almost everything in the game, the launcher and the tooling is theirs. Perforizon adds the performance work, the auto-tune, the PvP module and the wiki, and keeps following Horizon: see [Keeping up with Horizon](11-keeping-up-with-horizon.md).

## Licence

The repository's `LICENSE` is the MIT licence: "Copyright (c) 2026 Ludovic Marie and the Horizon contributors". Perforizon keeps it unchanged. If you redistribute Perforizon, keep the licence and the notices.

## Fonts

All under the SIL Open Font License 1.1, with the licence texts in `crates/horizon-theme/fonts/`:

- Geist and Geist Mono, by The Geist Project Authors. Geist Mono is the launcher's font.
- Bricolage Grotesque, by The Bricolage Grotesque Project Authors (its Cyrillic part comes from Geist).
- Pixelify Sans, by The Pixelify Sans Project Authors.

## Third-party notices

The full list, with licences, is `THIRD_PARTY_NOTICES.md` in the repository. In short:

| Component | Used for | Licence |
| --- | --- | --- |
| wgpu-hal and naga (gfx-rs developers) | graphics layer and shader translation, with fixes applied | MIT or Apache-2.0 |
| libmimalloc-sys and mimalloc | memory allocator | MIT |
| Pixelarticons (Gerrit Halfmann) | pixel icons | MIT |
| Hugeicons, free set | glass icons | MIT |
| Simple Icons | the Discord logo, on its button only | CC0 1.0 |
| AMD FidelityFX SDK | WGSL ports of the FSR 3.1 frame generation shaders | MIT |
| Mod loader logos | shown for the loader an instance uses | each project's own terms |
| CIT Resewn, FusionExamples, BlueMap | semantics and formats followed by the resource-pack and far-terrain code | see the notices file |

Trademarks belong to their owners: the logos only name the thing they stand for, and Perforizon is not affiliated with AMD, Discord or any mod loader.

## Disclaimer

Perforizon is an unofficial fan project. It is not affiliated with, endorsed by or sponsored by Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Studios. Perforizon contains no Minecraft code, textures or sounds: you must own Minecraft Java 26.3 and prepare the game files from your own copy ([Install](02-install.md)). The software is provided as is, without warranty, as the MIT licence says.

Back to [Home](01-home.md).
