use std::{fs, path::Path};

pub const FILE: &str = "horizon-pvp.txt";

trait Field: Sized + Copy {
    fn read(text: &str, fallback: Self) -> Self;
    fn write(self) -> String;
    fn step(self, min: Self, max: Self, reverse: bool) -> Self;
}
impl Field for bool {
    fn read(text: &str, fallback: Self) -> Self {
        match text {
            "true" => true,
            "false" => false,
            _ => fallback,
        }
    }
    fn write(self) -> String {
        self.to_string()
    }
    fn step(self, _: Self, _: Self, _: bool) -> Self {
        !self
    }
}
impl Field for i32 {
    fn read(text: &str, fallback: Self) -> Self {
        text.parse().unwrap_or(fallback)
    }
    fn write(self) -> String {
        self.to_string()
    }
    fn step(self, min: Self, max: Self, reverse: bool) -> Self {
        let step = ((max - min) / 10).max(1);
        let next = if reverse { self - step } else { self + step };
        if next > max {
            min
        } else if next < min {
            max
        } else {
            next
        }
    }
}
impl Field for f32 {
    fn read(text: &str, fallback: Self) -> Self {
        text.parse()
            .ok()
            .filter(|v: &f32| v.is_finite())
            .unwrap_or(fallback)
    }
    fn write(self) -> String {
        self.to_string()
    }
    fn step(self, min: Self, max: Self, reverse: bool) -> Self {
        let step = (max - min) / 10.0;
        let next = ((if reverse { self - step } else { self + step }) * 100.0).round() / 100.0;
        if next > max + step / 2.0 {
            min
        } else if next < min - step / 2.0 {
            max
        } else {
            next.clamp(min, max)
        }
    }
}
impl Field for u32 {
    fn read(text: &str, fallback: Self) -> Self {
        let digits = text.strip_prefix('#').or_else(|| text.strip_prefix("0x"));
        match digits {
            Some(hex) => u32::from_str_radix(hex, 16).unwrap_or(fallback),
            None => text.parse().unwrap_or(fallback),
        }
    }
    fn write(self) -> String {
        format!("#{self:08X}")
    }
    fn step(self, _: Self, _: Self, reverse: bool) -> Self {
        const PALETTE: [u32; 8] = [
            0xFFFF_FFFF,
            0xFF30_30FF,
            0xFFA0_20FF,
            0xFFFF_20FF,
            0x30FF_30FF,
            0x30FF_FFFF,
            0x3080_FFFF,
            0xFF30_FFFF,
        ];
        let at = PALETTE.iter().position(|c| *c == self);
        let next = match (at, reverse) {
            (Some(i), false) => (i + 1) % PALETTE.len(),
            (Some(i), true) => (i + PALETTE.len() - 1) % PALETTE.len(),
            (None, _) => 0,
        };
        PALETTE[next]
    }
}

macro_rules! pvp_settings {
    ($($(#[$doc:meta])* $field:ident: $ty:ty = $default:expr, $key:literal, $min:expr, $max:expr;)*) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct Pvp {
            $($(#[$doc])* pub $field: $ty,)*
        }
        impl Default for Pvp {
            fn default() -> Self {
                Self { $($field: $default,)* }
            }
        }
        impl Pvp {
            pub const KEYS: &'static [&'static str] = &[$($key,)*];
            pub fn set(&mut self, key: &str, value: &str) -> bool {
                match key {
                    $($key => {
                        let value = <$ty as Field>::read(value.trim(), self.$field);
                        let (min, max): ($ty, $ty) = ($min, $max);
                        self.$field = if min < max { value.clamp(min, max) } else { value };
                        true
                    })*
                    _ => false,
                }
            }
            pub fn cycle(&mut self, key: &str, reverse: bool) {
                match key {
                    $($key => {
                        let (min, max): ($ty, $ty) = ($min, $max);
                        self.$field = Field::step(self.$field, min, max, reverse);
                    })*
                    _ => {}
                }
            }
            pub fn get(&self, key: &str) -> Option<String> {
                match key {
                    $($key => Some(Field::write(self.$field)),)*
                    _ => None,
                }
            }
            pub fn serialize(&self) -> String {
                let mut text = String::new();
                $(text.push_str(&format!("{}:{}\n", $key, Field::write(self.$field)));)*
                text
            }
        }
    };
}

pvp_settings! {
    hit_prediction: bool = true, "hitPrediction", false, false;
    hit_prediction_sound: bool = true, "hitPredictionSound", false, false;
    crystal_optimizer: bool = true, "crystalOptimizer", false, false;
    anchor_optimizer: bool = true, "anchorOptimizer", false, false;
    elytra_optimizer: i32 = 1, "elytraOptimizer", 0, 1;
    toggle_sneak: bool = false, "toggleSneak", false, false;
    toggle_status: bool = true, "toggleStatus", false, false;
    toggle_status_corner: i32 = 0, "toggleStatusCorner", 0, 3;
    hitboxes: bool = false, "hitboxes", false, false;
    hitbox_color: u32 = 0xFFFFFFFF, "hitboxColor", 0, 0;
    hitbox_width: f32 = 2.0, "hitboxWidth", 0.5, 8.0;
    hitbox_rainbow: bool = false, "hitboxRainbow", false, false;
    hitbox_target_color: u32 = 0xFF3030FF, "hitboxTargetColor", 0, 0;
    hitbox_target_distance: f32 = 6.0, "hitboxTargetDistance", 1.0, 64.0;
    hitbox_players: bool = true, "hitboxPlayers", false, false;
    hitbox_monsters: bool = true, "hitboxMonsters", false, false;
    hitbox_projectiles: bool = true, "hitboxProjectiles", false, false;
    hitbox_items: bool = false, "hitboxItems", false, false;
    hitbox_eye_line: bool = false, "hitboxEyeLine", false, false;
    hitbox_look_length: f32 = 2.0, "hitboxLookLength", 0.5, 16.0;
    hitbox_behind_walls: bool = false, "hitboxBehindWalls", false, false;
    hitbox_behind_walls_fill: bool = false, "hitboxBehindWallsFill", false, false;
    hitbox_behind_walls_alpha: f32 = 0.35, "hitboxBehindWallsAlpha", 0.05, 1.0;
    hitbox_behind_walls_distance: f32 = 48.0, "hitboxBehindWallsDistance", 4.0, 256.0;
    projectile_trails: bool = false, "projectileTrails", false, false;
    projectile_trail_seconds: f32 = 2.0, "projectileTrailSeconds", 0.2, 10.0;
    projectile_trail_hitbox: bool = true, "projectileTrailHitbox", false, false;
    trail_arrow_color: u32 = 0xFFFF20FF, "trailArrowColor", 0, 0;
    trail_pearl_color: u32 = 0xFF30FFFF, "trailPearlColor", 0, 0;
    trail_potion_color: u32 = 0x30FFFFFF, "trailPotionColor", 0, 0;
    damage_flash: bool = false, "damageFlash", false, false;
    damage_flash_color: u32 = 0xFF0000FF, "damageFlashColor", 0, 0;
    damage_flash_alpha: f32 = 0.6, "damageFlashAlpha", 0.0, 1.0;
    damage_flash_seconds: f32 = 0.35, "damageFlashSeconds", 0.05, 2.0;
    crosshair_target_color: bool = false, "crosshairTargetColor", false, false;
    crosshair_color: u32 = 0xFF3030FF, "crosshairColor", 0, 0;
    crosshair_shape: i32 = 0, "crosshairShape", 0, 2;
    armor_hud: bool = false, "armorHud", false, false;
    armor_hud_corner: i32 = 3, "armorHudCorner", 0, 3;
    armor_hud_format: i32 = 1, "armorHudFormat", 0, 2;
    effect_hud: bool = false, "effectHud", false, false;
    effect_hud_corner: i32 = 1, "effectHudCorner", 0, 3;
    effect_hud_ticks: bool = false, "effectHudTicks", false, false;
    coordinates_hud: bool = false, "coordinatesHud", false, false;
    coordinates_corner: i32 = 0, "coordinatesCorner", 0, 3;
    coordinates_detailed: bool = false, "coordinatesDetailed", false, false;
    coordinates_direction: bool = true, "coordinatesDirection", false, false;
    direction_hud: bool = false, "directionHud", false, false;
    direction_corner: i32 = 0, "directionCorner", 0, 3;
    direction_degrees: bool = true, "directionDegrees", false, false;
    ping_hud: bool = false, "pingHud", false, false;
    ping_corner: i32 = 0, "pingCorner", 0, 3;
    ping_short: bool = false, "pingShort", false, false;
    combo_counter: bool = false, "comboCounter", false, false;
    combo_corner: i32 = 2, "comboCorner", 0, 3;
    combo_reset_seconds: f32 = 3.0, "comboResetSeconds", 0.5, 20.0;
    combo_alert: i32 = 10, "comboAlert", 0, 100;
    reach_display: bool = false, "reachDisplay", false, false;
    reach_corner: i32 = 2, "reachCorner", 0, 3;
    reach_seconds: f32 = 3.0, "reachSeconds", 0.5, 20.0;
    reach_decimals: i32 = 2, "reachDecimals", 0, 4;
    item_counter: bool = false, "itemCounter", false, false;
    item_counter_corner: i32 = 3, "itemCounterCorner", 0, 3;
    item_counter_totems: bool = true, "itemCounterTotems", false, false;
    item_counter_pearls: bool = true, "itemCounterPearls", false, false;
    item_counter_crystals: bool = true, "itemCounterCrystals", false, false;
    item_counter_low: i32 = 2, "itemCounterLow", 0, 64;
    explosion_countdown: bool = false, "explosionCountdown", false, false;
    explosion_ticks: bool = false, "explosionTicks", false, false;
    low_fire: bool = false, "lowFire", false, false;
    low_fire_height: f32 = 0.5, "lowFireHeight", 0.1, 1.0;
    low_shield: bool = false, "lowShield", false, false;
    low_shield_y: f32 = -0.15, "lowShieldY", -0.6, 0.6;
    small_totem: bool = false, "smallTotem", false, false;
    small_totem_scale: f32 = 0.5, "smallTotemScale", 0.1, 1.0;
    small_totem_hidden: bool = false, "smallTotemHidden", false, false;
    particle_filter: bool = false, "particleFilter", false, false;
    hide_explosion_particles: bool = false, "hideExplosionParticles", false, false;
    hide_potion_particles: bool = false, "hidePotionParticles", false, false;
    hide_smoke_particles: bool = false, "hideSmokeParticles", false, false;
    crit_multiplier: i32 = 1, "critMultiplier", 1, 5;
    time_changer: bool = false, "timeChanger", false, false;
    time_of_day: i32 = 6000, "timeOfDay", 0, 23999;
    time_speed: f32 = 0.0, "timeSpeed", 0.0, 100.0;
    no_speed_fov: bool = false, "noSpeedFov", false, false;
    sprint_fov_scale: f32 = 1.0, "sprintFovScale", 0.0, 1.0;
}

impl Pvp {
    pub fn caption(key: &str) -> String {
        let mut out = String::new();
        for (i, c) in key.chars().enumerate() {
            if c.is_uppercase() {
                out.push(' ');
            }
            out.extend(if i == 0 {
                c.to_uppercase().next()
            } else {
                Some(c)
            });
        }
        out
    }
    pub fn load(dir: &Path) -> Self {
        fs::read_to_string(dir.join(FILE))
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }
    pub fn parse(text: &str) -> Self {
        let mut pvp = Self::default();
        for line in text.lines() {
            if let Some((key, value)) = line.split_once(':') {
                pvp.set(key.trim(), value);
            }
        }
        pvp
    }
    pub fn save(&self, dir: &Path) {
        let path = dir.join(FILE);
        if *self == Self::default() && !path.exists() {
            return;
        }
        let _ = fs::write(path, self.serialize());
    }
}

pub fn flash_alpha(age: f32, seconds: f32, peak: f32) -> f32 {
    if seconds <= 0.0 || !(0.0..seconds).contains(&age) {
        return 0.0;
    }
    peak.clamp(0.0, 1.0) * (1.0 - age / seconds)
}

pub fn roman(n: i32) -> String {
    const TABLE: [(i32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut left = n.clamp(1, 3999);
    let mut out = String::new();
    for (value, text) in TABLE {
        while left >= value {
            out.push_str(text);
            left -= value;
        }
    }
    out
}

pub fn title_case(id: &str) -> String {
    let path = id.rsplit(':').next().unwrap_or(id);
    path.split(['_', '/'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn effect_clock(ticks: i32, as_ticks: bool) -> String {
    if ticks < 0 {
        return "**:**".to_owned();
    }
    if as_ticks {
        format!("{ticks}t")
    } else {
        format!("{}:{:02}", ticks / 1200, ticks / 20 % 60)
    }
}

pub fn countdown_text(fuse: f32, as_ticks: bool) -> String {
    let fuse = fuse.max(0.0);
    if as_ticks {
        format!("{}", fuse.ceil() as i32)
    } else {
        format!("{:.1}", fuse / 20.0)
    }
}

pub fn toggle_status(
    sprint_toggle: bool,
    sprint_down: bool,
    sneak_toggle: bool,
    sneak_down: bool,
    sprinting: bool,
    sneaking: bool,
) -> Vec<&'static str> {
    let mut out = Vec::new();
    if sneak_toggle && sneak_down {
        out.push("[Sneaking (Toggled)]");
    } else if sneaking && !sneak_toggle {
        out.push("[Sneaking (Held)]");
    }
    if sprint_toggle && sprint_down {
        out.push("[Sprinting (Toggled)]");
    } else if sprinting && !sprint_toggle {
        out.push("[Sprinting (Held)]");
    }
    out
}

pub fn crosshair_rects(shape: i32, cx: f32, cy: f32) -> Vec<[f32; 4]> {
    match shape {
        1 => vec![[cx - 1.0, cy - 1.0, 3.0, 3.0]],
        2 => vec![
            [cx - 5.0, cy - 5.0, 11.0, 1.0],
            [cx - 5.0, cy + 5.0, 11.0, 1.0],
            [cx - 5.0, cy - 4.0, 1.0, 9.0],
            [cx + 5.0, cy - 4.0, 1.0, 9.0],
        ],
        _ => vec![
            [cx - 5.0, cy, 4.0, 1.0],
            [cx + 2.0, cy, 4.0, 1.0],
            [cx, cy - 5.0, 1.0, 4.0],
            [cx, cy + 2.0, 1.0, 4.0],
        ],
    }
}

pub struct View {
    pub forward: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub tan_half_fov: f64,
    pub aspect: f64,
    pub width: f32,
    pub height: f32,
}

impl View {
    pub fn looking(forward: [f64; 3], fov_degrees: f32, width: f32, height: f32) -> Self {
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cross = |a: [f64; 3], b: [f64; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let norm = |a: [f64; 3]| {
            let l = dot(a, a).sqrt().max(1e-9);
            [a[0] / l, a[1] / l, a[2] / l]
        };
        let right = norm(cross(forward, [0.0, 1.0, 0.0]));
        let up = norm(cross(right, forward));
        Self {
            forward,
            right,
            up,
            tan_half_fov: (f64::from(fov_degrees).to_radians() / 2.0).tan(),
            aspect: f64::from(width / height.max(1.0)),
            width,
            height,
        }
    }

    pub fn project(&self, rel: [f64; 3]) -> Option<(f32, f32)> {
        let dot = |a: [f64; 3]| a[0] * rel[0] + a[1] * rel[1] + a[2] * rel[2];
        let depth = dot(self.forward);
        if depth < 0.1 {
            return None;
        }
        let x = dot(self.right) / (depth * self.tan_half_fov * self.aspect);
        let y = dot(self.up) / (depth * self.tan_half_fov);
        if x.abs() > 1.2 || y.abs() > 1.2 {
            return None;
        }
        Some((
            ((x + 1.0) / 2.0) as f32 * self.width,
            ((1.0 - y) / 2.0) as f32 * self.height,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailKind {
    Arrow,
    Pearl,
    Potion,
}

pub struct Trail {
    pub key: u64,
    pub kind: TrailKind,
    pub points: Vec<([f64; 3], f32)>,
}

#[derive(Default)]
pub struct Trails {
    pub list: Vec<Trail>,
    next_key: u64,
}

const UNTRACKED: u64 = 1 << 40;
const MATCH_DISTANCE: f64 = 3.0;
const MATCH_GAP: f32 = 0.2;

impl Trails {
    pub fn feed(&mut self, key: u64, kind: TrailKind, at: [f64; 3], now: f32) {
        match self.list.iter_mut().find(|t| t.key == key) {
            Some(t) => t.points.push((at, now)),
            None => self.list.push(Trail {
                key,
                kind,
                points: vec![(at, now)],
            }),
        }
    }

    pub fn feed_untracked(&mut self, kind: TrailKind, at: [f64; 3], now: f32) {
        let distance = |t: &Trail| {
            t.points.last().map_or(f64::MAX, |(p, _)| {
                (0..3).map(|i| (p[i] - at[i]).powi(2)).sum::<f64>().sqrt()
            })
        };
        let best = self
            .list
            .iter()
            .filter(|t| {
                t.key >= UNTRACKED
                    && t.kind == kind
                    && t.points
                        .last()
                        .is_some_and(|(_, seen)| *seen < now && now - *seen <= MATCH_GAP)
            })
            .map(|t| (t.key, distance(t)))
            .filter(|(_, d)| *d <= MATCH_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(key, _)| key);
        let key = best.unwrap_or_else(|| {
            self.next_key += 1;
            UNTRACKED + self.next_key
        });
        self.feed(key, kind, at, now);
    }

    pub fn prune(&mut self, now: f32, seconds: f32) {
        for t in &mut self.list {
            t.points.retain(|(_, at)| now - *at <= seconds);
        }
        self.list.retain(|t| !t.points.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_round_trips_and_stays_in_range() {
        let defaults = Pvp::default();
        assert_eq!(Pvp::parse(&defaults.serialize()), defaults);
        assert_eq!(Pvp::KEYS.len(), defaults.serialize().lines().count());
        let mut pvp = Pvp::default();
        assert!(pvp.set("critMultiplier", "99"));
        assert_eq!(pvp.crit_multiplier, 5);
        assert!(pvp.set("hitboxColor", "#11223344"));
        assert_eq!(pvp.hitbox_color, 0x1122_3344);
        assert!(pvp.set("hitboxWidth", "nope"));
        assert_eq!(pvp.hitbox_width, 2.0);
        assert!(!pvp.set("unknown", "1"));
        pvp.cycle("comboCounter", false);
        assert!(pvp.combo_counter);
        for _ in 0..30 {
            pvp.cycle("hitboxWidth", false);
            assert!((0.5..=8.0).contains(&pvp.hitbox_width));
            pvp.cycle("comboAlert", true);
            assert!((0..=100).contains(&pvp.combo_alert));
        }
        assert_eq!(Pvp::parse(&pvp.serialize()), pvp);
    }

    #[test]
    fn flash_fades_out_linearly_and_ends() {
        assert_eq!(flash_alpha(0.0, 0.5, 0.6), 0.6);
        assert!((flash_alpha(0.25, 0.5, 0.6) - 0.3).abs() < 1e-6);
        assert_eq!(flash_alpha(0.5, 0.5, 0.6), 0.0);
        assert_eq!(flash_alpha(-1.0, 0.5, 0.6), 0.0);
    }

    #[test]
    fn effect_text_matches_the_game_names() {
        assert_eq!(roman(1), "I");
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(9), "IX");
        assert_eq!(roman(14), "XIV");
        assert_eq!(title_case("fire_resistance"), "Fire Resistance");
        assert_eq!(title_case("minecraft:speed"), "Speed");
        assert_eq!(effect_clock(1200 + 20 * 5, false), "1:05");
        assert_eq!(effect_clock(45, true), "45t");
        assert_eq!(effect_clock(-1, false), "**:**");
        assert_eq!(countdown_text(80.0, false), "4.0");
        assert_eq!(countdown_text(7.2, true), "8");
        assert_eq!(countdown_text(-3.0, false), "0.0");
    }

    #[test]
    fn toggle_status_prefers_toggled_over_held() {
        assert_eq!(
            toggle_status(true, true, false, false, true, false),
            vec!["[Sprinting (Toggled)]"]
        );
        assert_eq!(
            toggle_status(false, true, true, true, true, true),
            vec!["[Sneaking (Toggled)]", "[Sprinting (Held)]"]
        );
        assert!(toggle_status(true, false, true, false, false, false).is_empty());
        assert_eq!(
            toggle_status(false, false, false, true, false, true),
            vec!["[Sneaking (Held)]"]
        );
        assert!(toggle_status(true, false, true, false, true, true).is_empty());
    }

    #[test]
    fn crosshair_shapes_stay_around_the_center() {
        for shape in 0..3 {
            let rects = crosshair_rects(shape, 100.0, 50.0);
            assert!(!rects.is_empty());
            for [x, y, w, h] in rects {
                assert!(x >= 94.0 && x + w <= 107.0 && y >= 44.0 && y + h <= 57.0);
            }
        }
    }

    #[test]
    fn projection_centers_what_is_straight_ahead() {
        let view = View::looking([0.0, 0.0, 1.0], 70.0, 400.0, 200.0);
        let (x, y) = view.project([0.0, 0.0, 10.0]).unwrap();
        assert!((x - 200.0).abs() < 1e-3 && (y - 100.0).abs() < 1e-3);
        assert!(view.project([0.0, 0.0, -5.0]).is_none());
        let (_, above) = view.project([0.0, 1.0, 10.0]).unwrap();
        assert!(above < 100.0);
        let (right, _) = view.project([-1.0, 0.0, 10.0]).unwrap();
        assert!(right > 200.0);
        assert!(view.project([100.0, 0.0, 1.0]).is_none());
    }

    #[test]
    fn trails_follow_untracked_projectiles_and_expire() {
        let mut trails = Trails::default();
        trails.feed_untracked(TrailKind::Potion, [0.0, 0.0, 0.0], 0.0);
        trails.feed_untracked(TrailKind::Potion, [0.0, 0.5, 0.0], 0.05);
        trails.feed_untracked(TrailKind::Potion, [50.0, 0.0, 0.0], 0.05);
        assert_eq!(trails.list.len(), 2);
        assert_eq!(trails.list[0].points.len(), 2);
        trails.feed(7, TrailKind::Arrow, [1.0, 1.0, 1.0], 0.1);
        trails.feed(7, TrailKind::Arrow, [2.0, 1.0, 1.0], 0.15);
        assert_eq!(trails.list.len(), 3);
        trails.prune(2.12, 2.0);
        assert_eq!(trails.list.len(), 1);
        assert_eq!(trails.list[0].points.len(), 1);
        trails.prune(5.0, 2.0);
        assert!(trails.list.is_empty());
    }
}
