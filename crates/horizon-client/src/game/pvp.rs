use super::Game;
use super::dragon::Crystal;
use horizon_client::pvp::{self as logic, TrailKind, Trails};
use std::time::{Duration, Instant};

const DUPLICATE_WINDOW: Duration = Duration::from_millis(700);
const MIN_HIT_GAP: Duration = Duration::from_millis(475);
const CRYSTAL_RESTORE: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub(super) struct PvpState {
    damage_at: Option<Instant>,
    trails: Trails,
    predicted: Vec<(u32, Instant)>,
    last_hit: Option<Instant>,
    sound_until: Option<Instant>,
    crystals: Vec<(Crystal, Instant)>,
    pub combo: u32,
    pub best_combo: u32,
    combo_at: Option<Instant>,
    pub reach: Option<(f64, Instant)>,
    started: Option<Instant>,
    aim: std::cell::Cell<(Option<u64>, Option<(usize, f64)>)>,
}

impl PvpState {
    pub(super) fn started_now() -> Self {
        Self {
            started: Some(Instant::now()),
            ..Self::default()
        }
    }
}

impl PvpState {
    pub(super) fn forget_crystal(&mut self, id: u32) {
        self.crystals.retain(|(c, _)| c.id != id);
    }
}

impl Game<'_> {
    pub(super) fn pvp_day_time(&self) -> i64 {
        let pvp = &self.settings.pvp;
        let Some(started) = self.pvp.started.filter(|_| pvp.time_changer) else {
            return self.world.day_time;
        };
        let moved = started.elapsed().as_secs_f32() * 20.0 * pvp.time_speed;
        (i64::from(pvp.time_of_day) + moved as i64).rem_euclid(24_000)
    }

    pub(super) fn pvp_predict_hit(&mut self, index: usize, distance: f64) {
        let now = Instant::now();
        let pvp = self.settings.pvp.clone();
        self.pvp.reach = Some((distance, now));
        let Some(mob) = self.mobs.get(index) else {
            return;
        };
        if !pvp.hit_prediction || !mob.alive() || mob.hurt_time > 0 {
            return;
        }
        if self.pvp.last_hit.is_some_and(|at| now - at < MIN_HIT_GAP) {
            return;
        }
        let id = mob.id;
        let (feet, width, height) = (
            [mob.position.x, mob.position.y, mob.position.z],
            mob.kind.width,
            mob.kind.height,
        );
        let falling = self
            .player
            .as_ref()
            .is_some_and(|p| p.fall_distance > 0.0 && !p.ground.on_ground);
        let sprinting = self.player.as_ref().is_some_and(|p| p.sprinting);
        let speed = self
            .own_attribute("minecraft:attack_speed")
            .unwrap_or_else(|| self.main_hand_attack_speed());
        let strength = self.hand_strength.scale(0.5, speed);
        if let Some(mob) = self.mobs.get_mut(index) {
            mob.hurt_time = 10;
            mob.limb_speed = 1.5;
        }
        self.pvp
            .predicted
            .retain(|(_, at)| now - *at < DUPLICATE_WINDOW);
        self.pvp.predicted.push((id, now));
        self.pvp.last_hit = Some(now);
        self.pvp.sound_until = Some(now + DUPLICATE_WINDOW);
        self.pvp.combo += 1;
        self.pvp.best_combo = self.pvp.best_combo.max(self.pvp.combo);
        self.pvp.combo_at = Some(now);
        let crit = strength > 0.9 && falling && !sprinting;
        if pvp.hit_prediction_sound {
            let sound = if crit {
                "minecraft:entity.player.attack.crit"
            } else if strength > 0.9 && sprinting {
                "minecraft:entity.player.attack.knockback"
            } else if strength > 0.9 {
                "minecraft:entity.player.attack.strong"
            } else {
                "minecraft:entity.player.attack.weak"
            };
            self.audio.play(sound, Some(feet), 1.0, 1.0);
        }
        if crit {
            self.critical_hit(None, id as i32, feet, width, height, false);
        }
    }

    pub(super) fn pvp_swallow_hurt(&self, id: u32) -> bool {
        let now = Instant::now();
        self.pvp
            .predicted
            .iter()
            .any(|(predicted, at)| *predicted == id && now - *at < DUPLICATE_WINDOW)
    }

    pub(super) fn pvp_swallow_sound(&self, name: &str) -> bool {
        name.starts_with("minecraft:entity.player.attack.")
            && self
                .pvp
                .sound_until
                .is_some_and(|until| Instant::now() < until)
    }

    pub(super) fn pvp_crystal_struck(&mut self, id: i32) {
        if !self.settings.pvp.crystal_optimizer {
            return;
        }
        let id = id as u32;
        if let Some(i) = self.crystals.iter().position(|c| c.id == id) {
            let crystal = self.crystals.remove(i);
            self.pvp
                .crystals
                .push((crystal, Instant::now() + CRYSTAL_RESTORE));
        }
    }

    pub(super) fn pvp_hurt_us(&mut self) {
        self.pvp.combo = 0;
        self.pvp.damage_at = Some(Instant::now());
    }

    fn pvp_aim(&self) -> Option<(usize, f64)> {
        let (frame, hit) = self.pvp.aim.get();
        if frame == Some(self.frames) {
            return hit;
        }
        let hit = self.mob_hit();
        self.pvp.aim.set((Some(self.frames), hit));
        hit
    }

    fn pvp_clock(&self) -> f32 {
        self.pvp.started.map_or(0.0, |s| s.elapsed().as_secs_f32())
    }

    pub(super) fn pvp_anchor_ghost(&mut self, pos: [i32; 3], seq: i32) -> bool {
        if !self.settings.pvp.anchor_optimizer || self.anchor_works() || self.integrated.is_some() {
            return false;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let glowstone = |s: Option<&horizon_client::items::ItemStack>| {
            s.is_some_and(|s| &*s.item == "minecraft:glowstone")
        };
        if !self.is_anchor(state)
            || self.anchor_charges(state) < 1
            || self.toggles.is_down(0)
            || glowstone(self.held())
            || glowstone(self.offhand.as_ref())
        {
            return false;
        }
        let block = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
        self.predict_blocks(
            &vec![(block, horizon_world::registry::BlockStateId(0))],
            seq,
        );
        true
    }

    fn pvp_collect_trails(&mut self) {
        let pvp = &self.settings.pvp;
        if !pvp.projectile_trails {
            self.pvp.trails.list.clear();
            return;
        }
        let seconds = pvp.projectile_trail_seconds;
        let now = self.pvp_clock();
        let mut tracked = Vec::new();
        for p in &self.creatures.projectiles {
            use horizon_client::projectiles::ProjectileKind as K;
            if p.stuck {
                continue;
            }
            let kind = match p.kind {
                K::Arrow { .. } => TrailKind::Arrow,
                K::EnderPearl => TrailKind::Pearl,
                K::Potion { .. } => TrailKind::Potion,
                _ => continue,
            };
            tracked.push((p.id, kind, [p.position.x, p.position.y, p.position.z]));
        }
        let thrown: Vec<[f64; 3]> = self
            .potions
            .thrown_entities()
            .into_iter()
            .map(|(_, at, _)| [at.x, at.y, at.z])
            .collect();
        let trails = &mut self.pvp.trails;
        for (id, kind, at) in tracked {
            if id != 0 {
                trails.feed(u64::from(id), kind, at, now);
            } else {
                trails.feed_untracked(kind, at, now);
            }
        }
        for at in thrown {
            trails.feed_untracked(TrailKind::Potion, at, now);
        }
        trails.prune(now, seconds);
    }

    pub(super) fn pvp_tick(&mut self) {
        self.pvp_collect_trails();
        let now = Instant::now();
        if !self.pvp.crystals.is_empty() {
            let (due, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pvp.crystals)
                .into_iter()
                .partition(|(_, until)| *until <= now);
            self.pvp.crystals = waiting;
            self.crystals.extend(due.into_iter().map(|(c, _)| c));
        }
        let reset = Duration::from_secs_f32(self.settings.pvp.combo_reset_seconds);
        if self.pvp.combo_at.is_some_and(|at| now - at > reset) {
            self.pvp.combo = 0;
            self.pvp.combo_at = None;
        }
        self.pvp
            .predicted
            .retain(|(_, at)| now - *at < DUPLICATE_WINDOW);
    }
}

fn corner(index: i32) -> horizon_client::extras::Corner {
    horizon_client::extras::Corner::ALL[index.clamp(0, 3) as usize]
}

fn facing(yaw: f32) -> &'static str {
    const NAMES: [&str; 8] = ["S", "SW", "W", "NW", "N", "NE", "E", "SE"];
    NAMES[(((yaw.rem_euclid(360.0) + 22.5) / 45.0) as usize) % 8]
}

fn health_color(fraction: f32) -> u32 {
    match fraction {
        f if f > 0.6 => 0x55FF55,
        f if f > 0.3 => 0xFFFF55,
        _ => 0xFF5555,
    }
}

impl Game<'_> {
    pub(super) fn draw_pvp_hud(
        &mut self,
        batch: &mut horizon_client::hud::HudBatch,
        corners: &mut horizon_client::extras_hud::Corners,
    ) {
        if self.screen != super::Screen::Playing || self.title.is_some() {
            return;
        }
        let pvp = self.settings.pvp.clone();
        let mut panels: Vec<(i32, Vec<(String, u32)>)> = Vec::new();
        let now = Instant::now();
        if pvp.armor_hud {
            let mut lines = Vec::new();
            for stack in self.armor.iter().flatten() {
                let max = self
                    .res
                    .items()
                    .get(&stack.item)
                    .map_or(0, |item| item.max_damage);
                if max == 0 {
                    continue;
                }
                let left = max.saturating_sub(stack.damage);
                let fraction = left as f32 / max as f32;
                let name = stack
                    .item
                    .trim_start_matches("minecraft:")
                    .replace('_', " ");
                let value = match pvp.armor_hud_format {
                    0 => format!("{left}/{max}"),
                    1 => format!("{:.0}%", fraction * 100.0),
                    _ => {
                        let filled = (fraction * 10.0).round() as usize;
                        format!("{}{}", "|".repeat(filled), ".".repeat(10 - filled))
                    }
                };
                lines.push((format!("{name} {value}"), health_color(fraction)));
            }
            panels.push((pvp.armor_hud_corner, lines));
        }
        if pvp.effect_hud {
            let lines = self
                .potions
                .effects
                .list
                .iter()
                .filter(|e| e.visible)
                .map(|e| {
                    let name = horizon_client::lang::get_or(
                        &horizon_client::brewing::description_id(e.id),
                        &logic::title_case(e.id),
                    );
                    let level = if e.amplifier > 0 {
                        format!(" {}", logic::roman(e.amplifier + 1))
                    } else {
                        String::new()
                    };
                    let color = horizon_client::brewing::effect(e.id)
                        .map_or(0xFFFFFF, |d| d.color & 0xFF_FFFF);
                    (
                        format!(
                            "{name}{level} {}",
                            logic::effect_clock(e.duration, pvp.effect_hud_ticks)
                        ),
                        color,
                    )
                })
                .collect();
            panels.push((pvp.effect_hud_corner, lines));
        }
        if let Some(p) = self.player.as_ref() {
            let (x, y, z) = (p.position.x, p.position.y, p.position.z);
            if pvp.coordinates_hud {
                let mut text = if pvp.coordinates_detailed {
                    format!("X {x:.3} Y {y:.3} Z {z:.3}")
                } else {
                    format!("{x:.0} {y:.0} {z:.0}")
                };
                if pvp.coordinates_direction {
                    text = format!("{text} {}", facing(self.yaw));
                }
                panels.push((pvp.coordinates_corner, vec![(text, 0xFFFFFF)]));
            }
            if pvp.direction_hud {
                let text = if pvp.direction_degrees {
                    format!("{} {:.0}", facing(self.yaw), self.yaw.rem_euclid(360.0))
                } else {
                    facing(self.yaw).to_owned()
                };
                panels.push((pvp.direction_corner, vec![(text, 0xFFFFFF)]));
            }
        }
        if pvp.ping_hud {
            let fps = self.frame_reading().0;
            let ping = self.remote.as_ref().and_then(|r| {
                r.tab_players
                    .get(&r.profile())
                    .map(|t| t.latency)
                    .filter(|l| *l >= 0)
            });
            let fps_color = match fps {
                f if f >= 120 => 0x55FF55,
                f if f >= 60 => 0xFFFF55,
                _ => 0xFF5555,
            };
            let mut lines = vec![(
                if pvp.ping_short {
                    format!("{fps}")
                } else {
                    format!("{fps} FPS")
                },
                fps_color,
            )];
            if let Some(ping) = ping {
                let color = match ping {
                    p if p <= 60 => 0x55FF55,
                    p if p <= 150 => 0xFFFF55,
                    _ => 0xFF5555,
                };
                lines.push((
                    if pvp.ping_short {
                        format!("{ping}")
                    } else {
                        format!("{ping} ms")
                    },
                    color,
                ));
            }
            panels.push((pvp.ping_corner, lines));
        }
        if self.settings.horizon.latency_hud {
            let l = horizon_client::latency::snapshot();
            let color = match l.gpu_avg {
                ms if ms <= 8.0 => 0x55FF55,
                ms if ms <= 20.0 => 0xFFFF55,
                _ => 0xFF5555,
            };
            panels.push((
                pvp.ping_corner,
                vec![
                    (
                        format!("Input {:.1} ms (max {:.1})", l.gpu_avg, l.gpu_max),
                        color,
                    ),
                    (
                        format!("Present {:.1} ms (max {:.1})", l.submit_avg, l.submit_max),
                        0xFFFFFF,
                    ),
                ],
            ));
        }
        if pvp.combo_counter && self.pvp.combo > 0 {
            let mut text = format!("Combo {}", self.pvp.combo);
            let color = if pvp.combo_alert > 0 && self.pvp.combo as i32 >= pvp.combo_alert {
                text.push('!');
                0xFFAA00
            } else {
                0xFFFFFF
            };
            panels.push((pvp.combo_corner, vec![(text, color)]));
        }
        if pvp.reach_display
            && let Some((distance, at)) = self.pvp.reach
            && now.duration_since(at).as_secs_f32() < pvp.reach_seconds
        {
            let decimals = pvp.reach_decimals.clamp(0, 4) as usize;
            panels.push((
                pvp.reach_corner,
                vec![(format!("Reach {distance:.decimals$}"), 0xFFFFFF)],
            ));
        }
        if pvp.item_counter {
            let wanted = [
                (
                    pvp.item_counter_totems,
                    "minecraft:totem_of_undying",
                    "Totems",
                ),
                (pvp.item_counter_pearls, "minecraft:ender_pearl", "Pearls"),
                (
                    pvp.item_counter_crystals,
                    "minecraft:end_crystal",
                    "Crystals",
                ),
            ];
            let mut lines = Vec::new();
            for (on, id, label) in wanted {
                if !on {
                    continue;
                }
                let count: u32 = self
                    .inventory
                    .iter()
                    .chain(std::iter::once(&self.offhand))
                    .flatten()
                    .filter(|s| &*s.item == id)
                    .map(|s| s.count)
                    .sum();
                let low = (count as i32) < pvp.item_counter_low;
                lines.push((
                    format!("{label} {count}"),
                    if low { 0xFF5555 } else { 0xFFFFFF },
                ));
            }
            panels.push((pvp.item_counter_corner, lines));
        }
        if pvp.toggle_status {
            let sprinting = self.player.as_ref().is_some_and(|p| p.sprinting);
            let sneaking = self
                .player
                .as_ref()
                .is_some_and(|p| p.pose == horizon_client::pose::Pose::Crouching);
            let lines = logic::toggle_status(
                self.settings.toggle_sprint,
                self.toggles.is_down(1),
                self.settings.toggle_crouch,
                self.toggles.is_down(0),
                sprinting,
                sneaking,
            )
            .into_iter()
            .map(|t| (t.to_owned(), 0xFFFFFF))
            .collect();
            panels.push((pvp.toggle_status_corner, lines));
        }
        self.draw_pvp_overlays(batch, &pvp);
        let atlas = &self.gui.atlas;
        for (index, lines) in panels {
            if lines.is_empty() {
                continue;
            }
            let lines: Vec<(&str, u32)> = lines.iter().map(|(t, c)| (t.as_str(), *c)).collect();
            horizon_client::extras_hud::draw_lines(batch, atlas, corners, corner(index), &lines);
        }
    }
}

fn argb(rgba: u32) -> u32 {
    rgba.rotate_right(8)
}

fn rainbow(seconds: f32) -> u32 {
    let h = (seconds * 0.25).fract() * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let byte = |v: f32| (v * 255.0) as u32;
    0xFF00_0000 | byte(r) << 16 | byte(g) << 8 | byte(b)
}

struct MobBox {
    index: usize,
    pos: [f64; 3],
    min: [f64; 3],
    max: [f64; 3],
    color: u32,
}

impl Game<'_> {
    fn pvp_mob_boxes(&self, alpha: f32) -> Vec<MobBox> {
        let pvp = &self.settings.pvp;
        let lerp = |a: f64, b: f64| a + f64::from(alpha) * (b - a);
        let aimed = self
            .pvp_aim()
            .filter(|(_, d)| *d <= f64::from(pvp.hitbox_target_distance))
            .map(|(i, _)| i);
        let base = if pvp.hitbox_rainbow {
            rainbow(self.pvp_clock())
        } else {
            argb(pvp.hitbox_color)
        };
        let eye_player = self.player.as_ref().map(|p| p.position);
        let mut out = Vec::new();
        for (index, m) in self.mobs.iter().enumerate() {
            if m.removed || m.invisible {
                continue;
            }
            let player = m.kind.id == "minecraft:player";
            if (player && !pvp.hitbox_players) || (!player && !pvp.hitbox_monsters) {
                continue;
            }
            let pos = [
                lerp(m.previous.x, m.position.x),
                lerp(m.previous.y, m.position.y),
                lerp(m.previous.z, m.position.z),
            ];
            if pvp.hitbox_behind_walls
                && eye_player.is_some_and(|p| {
                    let (dx, dy, dz) = (pos[0] - p.x, pos[1] - p.y, pos[2] - p.z);
                    (dx * dx + dy * dy + dz * dz).sqrt()
                        > f64::from(pvp.hitbox_behind_walls_distance)
                })
            {
                continue;
            }
            let half = f64::from(m.width()) / 2.0;
            out.push(MobBox {
                index,
                pos,
                min: [pos[0] - half, pos[1], pos[2] - half],
                max: [pos[0] + half, pos[1] + f64::from(m.height()), pos[2] + half],
                color: if aimed == Some(index) {
                    argb(pvp.hitbox_target_color)
                } else {
                    base
                },
            });
        }
        out
    }

    pub(super) fn pvp_hitboxes(&self, alpha: f32, g: &mut horizon_client::gizmos::Gizmos) {
        let pvp = &self.settings.pvp;
        if pvp.hitboxes {
            self.pvp_mob_lines(alpha, g);
            if pvp.hitbox_projectiles {
                self.pvp_projectile_boxes(alpha, g);
            }
        }
        if pvp.projectile_trails {
            self.pvp_trail_lines(alpha, g);
        }
    }

    fn pvp_projectile_boxes(&self, alpha: f32, g: &mut horizon_client::gizmos::Gizmos) {
        let pvp = &self.settings.pvp;
        let lerp = |a: f64, b: f64| a + f64::from(alpha) * (b - a);
        for p in &self.creatures.projectiles {
            let size = p.kind.size();
            let half = size / 2.0;
            let at = [
                lerp(p.previous.x, p.position.x),
                lerp(p.previous.y, p.position.y),
                lerp(p.previous.z, p.position.z),
            ];
            g.cuboid(
                [at[0] - half, at[1], at[2] - half],
                [at[0] + half, at[1] + size, at[2] + half],
                argb(pvp.hitbox_color),
                pvp.hitbox_width,
            );
        }
    }

    fn pvp_trail_lines(&self, alpha: f32, g: &mut horizon_client::gizmos::Gizmos) {
        let pvp = &self.settings.pvp;
        let now = self.pvp_clock();
        let seconds = pvp.projectile_trail_seconds.max(0.1);
        for trail in &self.pvp.trails.list {
            let rgb = argb(match trail.kind {
                TrailKind::Arrow => pvp.trail_arrow_color,
                TrailKind::Pearl => pvp.trail_pearl_color,
                TrailKind::Potion => pvp.trail_potion_color,
            }) & 0xFF_FFFF;
            let mut points = trail.points.clone();
            if trail.key < 1 << 32
                && let Some(p) = self
                    .creatures
                    .projectiles
                    .iter()
                    .find(|p| u64::from(p.id) == trail.key && !p.stuck)
                && let Some(last) = points.last_mut()
            {
                let lerp = |a: f64, b: f64| a + f64::from(alpha) * (b - a);
                last.0 = [
                    lerp(p.previous.x, p.position.x),
                    lerp(p.previous.y, p.position.y),
                    lerp(p.previous.z, p.position.z),
                ];
            }
            for pair in points.windows(2) {
                let fade = (1.0 - (now - pair[1].1) / seconds).clamp(0.0, 1.0);
                let a = (fade * 255.0) as u32;
                if a > 0 {
                    g.line(pair[0].0, pair[1].0, a << 24 | rgb, 3.0);
                }
            }
            if pvp.projectile_trail_hitbox
                && let Some((p, at)) = trail.points.last()
                && now - *at < 0.12
            {
                let half = if trail.kind == TrailKind::Arrow {
                    0.25
                } else {
                    0.125
                };
                g.cuboid(
                    [p[0] - half, p[1] - half, p[2] - half],
                    [p[0] + half, p[1] + half, p[2] + half],
                    0xFF00_0000 | rgb,
                    pvp.hitbox_width,
                );
            }
        }
    }

    fn pvp_mob_lines(&self, alpha: f32, g: &mut horizon_client::gizmos::Gizmos) {
        let pvp = &self.settings.pvp;
        let lerp = |a: f64, b: f64| a + f64::from(alpha) * (b - a);
        for b in self.pvp_mob_boxes(alpha) {
            let (min, max, pos) = (b.min, b.max, b.pos);
            if pvp.hitbox_behind_walls {
                g.cuboid_on_top(min, max, b.color, pvp.hitbox_width);
            } else {
                g.cuboid(min, max, b.color, pvp.hitbox_width);
            }
            if !pvp.hitbox_eye_line {
                continue;
            }
            let m = &self.mobs[b.index];
            let eye = pos[1] + f64::from(m.eye_height() as f32);
            let d = 0.01;
            g.cuboid(
                [min[0], eye - d, min[2]],
                [max[0], eye + d, max[2]],
                0xFFFF_0000,
                pvp.hitbox_width,
            );
            let pitch = lerp(f64::from(m.previous_head_pitch), f64::from(m.head_pitch)) as f32;
            let yaw = lerp(f64::from(m.previous_head_yaw), f64::from(m.head_yaw)) as f32;
            let (sy, cy) = yaw.to_radians().sin_cos();
            let (sp, cp) = pitch.to_radians().sin_cos();
            let length = f64::from(pvp.hitbox_look_length);
            let start = [pos[0], eye, pos[2]];
            let end = [
                start[0] - f64::from(sy * cp) * length,
                start[1] - f64::from(sp) * length,
                start[2] + f64::from(cy * cp) * length,
            ];
            g.arrow(start, end, 0xFF00_00FF, pvp.hitbox_width);
        }
    }

    pub(super) fn pvp_fill_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f32,
        out: &mut Vec<horizon_client::mesher::Vertex>,
    ) {
        let pvp = &self.settings.pvp;
        if !(pvp.hitboxes && pvp.hitbox_behind_walls && pvp.hitbox_behind_walls_fill) {
            return;
        }
        let a = (pvp.hitbox_behind_walls_alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let mut white = horizon_client::hud::HudBatch::new([1, 1]);
        for b in self.pvp_mob_boxes(alpha) {
            let c = b.color;
            white.vertices.clear();
            white.fill(
                &self.gui.atlas,
                0.0,
                0.0,
                1.0,
                1.0,
                horizon_client::hud::rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, a),
            );
            let Some(v) = white.vertices.first().copied() else {
                continue;
            };
            for quad in horizon_client::gizmos::cuboid_fill_quads(b.min, b.max) {
                for corner in quad {
                    out.push(horizon_client::mesher::Vertex {
                        normal: [0; 4],
                        pos: (glam::DVec3::from_array(corner) - camera)
                            .as_vec3()
                            .to_array(),
                        uv: v.uv,
                        layer: 2,
                        color: v.color,
                        light: 0,
                        glint_uv: [0.0, 0.0],
                    });
                }
            }
        }
    }

    fn draw_pvp_overlays(
        &self,
        batch: &mut horizon_client::hud::HudBatch,
        pvp: &horizon_client::pvp::Pvp,
    ) {
        use horizon_client::hud::{HudBatch, rgba};
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        let channels = |c: u32| ((c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8);
        if pvp.damage_flash
            && let Some(at) = self.pvp.damage_at
        {
            let a = logic::flash_alpha(
                at.elapsed().as_secs_f32(),
                pvp.damage_flash_seconds,
                pvp.damage_flash_alpha,
            );
            if a > 0.0 {
                let (r, g, b) = channels(pvp.damage_flash_color);
                batch.fill(atlas, 0.0, 0.0, w, h, rgba(r, g, b, (a * 255.0) as u8));
            }
        }
        let first_person = self.camera_type.first_person();
        if pvp.crosshair_target_color
            && first_person
            && self.pvp_aim().is_some_and(|(i, d)| {
                self.mobs[i].kind.id == "minecraft:player" && self.before_block(d)
            })
        {
            let (r, g, b) = channels(pvp.crosshair_color);
            let cx = ((w - 15.0) / 2.0).floor() + 7.0;
            let cy = ((h - 15.0) / 2.0).floor() + 7.0;
            for [x, y, rw, rh] in logic::crosshair_rects(pvp.crosshair_shape, cx, cy) {
                batch.fill(atlas, x, y, rw, rh, rgba(r, g, b, 255));
            }
        }
        if !(pvp.explosion_countdown && first_person) {
            return;
        }
        let partial = self.clock.fraction() as f32;
        let Some(eye) = self.eye_position() else {
            return;
        };
        let view = logic::View::looking(
            super::view_vector(self.yaw, self.pitch),
            self.fov_degrees(partial, eye),
            w,
            h,
        );
        let t = f64::from(partial);
        for tnt in &self.tnts {
            let at = [
                tnt.previous.x + t * (tnt.position.x - tnt.previous.x),
                tnt.previous.y + t * (tnt.position.y - tnt.previous.y) + 1.4,
                tnt.previous.z + t * (tnt.position.z - tnt.previous.z),
            ];
            let rel = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
            if rel.iter().map(|v| v * v).sum::<f64>() > 64.0 * 64.0 {
                continue;
            }
            let Some((x, y)) = view.project(rel) else {
                continue;
            };
            let fuse = tnt.fuse as f32 - partial + 1.0;
            let text = logic::countdown_text(fuse, pvp.explosion_ticks);
            let color = if fuse <= 20.0 {
                rgba(255, 85, 85, 255)
            } else {
                rgba(255, 170, 0, 255)
            };
            let tw = HudBatch::text_width(atlas, &text);
            batch.text(
                atlas,
                &text,
                (x - tw / 2.0).round(),
                (y - 4.0).round(),
                color,
                true,
            );
        }
    }
}
