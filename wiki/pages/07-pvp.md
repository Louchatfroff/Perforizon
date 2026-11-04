---
title: PvP module
section: Use
summary: Hit prediction, hitboxes, trails and HUD overlays, with every option group and how to set it.
---

# PvP module

The PvP module is Perforizon's own addition. Its options page is `Horizon > PvP` in the game's options. Settings live in `horizon-pvp.txt` in the game folder, one `key:value` per line.

**Status:** the latency features (hit prediction and the optimizers) are on by default and are untested against a live server so far. Everything else is a visual overlay, off by default.

## Set an option

In the options screen, or by file, for example:

```text
hitboxes:true
hitPrediction:false
```

Or live from the command line:

```sh
horizon-ctl pvp comboCounter true     # set
horizon-ctl pvp comboCounter          # read
```

Colours are written as `#RRGGBBAA`. Corner options take 0 to 3. See [Settings](05-settings.md) for the graphics options.

## Latency

Defaults on. See also [Latency](06-latency.md).

| Key | Default | Effect |
| --- | --- | --- |
| `hitPrediction` | on | Swing before the server confirms a hit. |
| `hitPredictionSound` | on | Plays the hit sound at once too. |
| `crystalOptimizer` | on | Hides the crystal at once; restored if the server rejects it. |
| `anchorOptimizer` | on | The same for respawn anchors. |
| `elytraOptimizer` | 1 | Elytra stop (0 or 1). |

## Toggle sprint and sneak

| Key | Default | Effect |
| --- | --- | --- |
| `toggleSneak` | off | Sneak as a toggle. |
| `toggleStatus` | on | Status line showing the toggle state. |
| `toggleStatusCorner` | 0 | Where the status line sits. |

## Hitboxes

Key prefix `hitbox`: `hitboxes` (master, off), `hitboxColor`, `hitboxWidth` (default 2), `hitboxRainbow`, `hitboxTargetColor` and `hitboxTargetDistance` (default 6), the filters `hitboxPlayers`, `hitboxMonsters`, `hitboxProjectiles` (on) and `hitboxItems` (off), `hitboxEyeLine` with `hitboxLookLength`, and the behind-walls group `hitboxBehindWalls`, `hitboxBehindWallsFill`, `hitboxBehindWallsAlpha` (0.35) and `hitboxBehindWallsDistance` (48).

## Projectile trails and damage flash

- `projectileTrails` (off), `projectileTrailSeconds` (2), `projectileTrailHitbox` (on), and the colours `trailArrowColor`, `trailPearlColor`, `trailPotionColor`.
- `damageFlash` (off) with `damageFlashColor`, `damageFlashAlpha` (0.6) and `damageFlashSeconds` (0.35).

## Crosshair

`crosshairTargetColor` (off) changes the colour over a target, with `crosshairColor`; `crosshairShape` takes 0 to 2.

## HUD overlays

Each overlay has a switch and a corner (0 to 3):

| Overlay | Switch | Notable keys |
| --- | --- | --- |
| Armor | `armorHud` | `armorHudCorner`, `armorHudFormat` (0 to 2) |
| Effects | `effectHud` | `effectHudCorner`, `effectHudTicks` |
| Coordinates | `coordinatesHud` | `coordinatesCorner`, `coordinatesDetailed`, `coordinatesDirection` |
| Direction | `directionHud` | `directionCorner`, `directionDegrees` |
| Ping | `pingHud` | `pingCorner`, `pingShort` |
| Combo | `comboCounter` | `comboCorner`, `comboResetSeconds` (3), `comboAlert` (10) |
| Reach | `reachDisplay` | `reachCorner`, `reachSeconds` (3), `reachDecimals` (2) |
| Item counter | `itemCounter` | `itemCounterCorner`, `itemCounterTotems`, `itemCounterPearls`, `itemCounterCrystals`, `itemCounterLow` (2) |
| Explosion countdown | `explosionCountdown` | `explosionTicks` |

The latency counter of [Latency](06-latency.md) uses the same corner as the ping counter.

## View and particles

- `lowFire` (with `lowFireHeight`), `lowShield` (with `lowShieldY`), `smallTotem` (with `smallTotemScale` and `smallTotemHidden`).
- `particleFilter` with `hideExplosionParticles`, `hidePotionParticles`, `hideSmokeParticles`.
- `critMultiplier` (1 to 5), `timeChanger` with `timeOfDay` and `timeSpeed`, `noSpeedFov` with `sprintFovScale`.

## Servers and fairness

Most of these overlays are cosmetic and client side. Whether a given server allows a feature is that server's rule: check it before you use hitboxes, prediction or the time changer there. Keys are in `crates/horizon-client-render/src/pvp.rs`.
