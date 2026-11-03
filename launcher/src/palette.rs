use horizon_theme::contrast;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub name: &'static str,
    pub label: &'static str,
    pub dark: bool,
    pub bg: u32,
    pub bg_alt: u32,
    pub border: u32,
    pub border_focus: u32,
    pub fg: u32,
    pub fg_dim: u32,
    pub accent: u32,
    pub accent_dim: u32,
    pub ok: u32,
    pub warn: u32,
    pub err: u32,
    pub selection_bg: u32,
    pub selection_fg: u32,
}

const fn p(name: &'static str, label: &'static str, dark: bool, c: [u32; 13]) -> Palette {
    Palette {
        name,
        label,
        dark,
        bg: c[0],
        bg_alt: c[1],
        border: c[2],
        border_focus: c[3],
        fg: c[4],
        fg_dim: c[5],
        accent: c[6],
        accent_dim: c[7],
        ok: c[8],
        warn: c[9],
        err: c[10],
        selection_bg: c[11],
        selection_fg: c[12],
    }
}

pub const DEFAULT: &str = "grayscale";

#[rustfmt::skip]
pub const ALL: [Palette; 16] = [
    p("grayscale", "Grayscale", true, [0x0c0c0c, 0x161616, 0x666666, 0xd0d0d0, 0xe6e6e6, 0x9a9a9a, 0xf2f2f2, 0x8c8c8c, 0xd8d8d8, 0xbdbdbd, 0xffffff, 0xf2f2f2, 0x0c0c0c]),
    p("tokyo-night", "Tokyo Night", true, [0x1a1b26, 0x16161e, 0x60688a, 0x7aa2f7, 0xc0caf5, 0x848cb5, 0x7aa2f7, 0x5f6790, 0x9ece6a, 0xe0af68, 0xf7768e, 0x7aa2f7, 0x1a1b26]),
    p("tokyo-night-storm", "Tokyo Night Storm", true, [0x24283b, 0x1f2335, 0x6b7396, 0x7aa2f7, 0xc0caf5, 0x8890b8, 0x7aa2f7, 0x6b7398, 0x9ece6a, 0xe0af68, 0xf7768e, 0x7aa2f7, 0x24283b]),
    p("catppuccin-latte", "Catppuccin Latte", false, [0xeff1f5, 0xe6e9ef, 0x7e8195, 0x1e66f5, 0x4c4f69, 0x63667a, 0x1c5ee1, 0x7c7f93, 0x2f7620, 0x915c13, 0xce0f38, 0x1c5ee1, 0xeff1f5]),
    p("catppuccin-frappe", "Catppuccin Frappe", true, [0x303446, 0x292c3c, 0x79809b, 0x8caaee, 0xc6d0f5, 0xa5adce, 0x8caaee, 0x797f99, 0xa6d189, 0xe5c890, 0xe78284, 0x8caaee, 0x303446]),
    p("catppuccin-macchiato", "Catppuccin Macchiato", true, [0x24273a, 0x1e2030, 0x6d738d, 0x8aadf4, 0xcad3f5, 0xa5adcb, 0x8aadf4, 0x6e738d, 0xa6da95, 0xeed49f, 0xed8796, 0x8aadf4, 0x24273a]),
    p("catppuccin-mocha", "Catppuccin Mocha", true, [0x1e1e2e, 0x181825, 0x686c82, 0x89b4fa, 0xcdd6f4, 0xa6adc8, 0x89b4fa, 0x6c7086, 0xa6e3a1, 0xf9e2af, 0xf38ba8, 0x89b4fa, 0x1e1e2e]),
    p("gruvbox-dark", "Gruvbox Dark", true, [0x282828, 0x1d2021, 0x7b7264, 0xfabd2f, 0xebdbb2, 0xa89984, 0xfabd2f, 0x928374, 0xb8bb26, 0xd79921, 0xfb5844, 0xfabd2f, 0x282828]),
    p("nord", "Nord", true, [0x2e3440, 0x3b4252, 0x8a92a2, 0x88c0d0, 0xd8dee9, 0xa7b0c0, 0x88c0d0, 0x81a1c1, 0xa3be8c, 0xebcb8b, 0xd9a2a7, 0x88c0d0, 0x2e3440]),
    p("dracula", "Dracula", true, [0x282a36, 0x21222c, 0x737582, 0xbd93f9, 0xf8f8f2, 0x8e9cd0, 0xbd93f9, 0x6575a6, 0x50fa7b, 0xf1fa8c, 0xff5858, 0xbd93f9, 0x282a36]),
    p("rose-pine", "Rose Pine", true, [0x191724, 0x1f1d2e, 0x6d6a7f, 0xc4a7e7, 0xe0def4, 0x908caa, 0xc4a7e7, 0x6e6a86, 0x9ccfd8, 0xf6c177, 0xeb6f92, 0xc4a7e7, 0x191724]),
    p("solarized-dark", "Solarized Dark", true, [0x002b36, 0x073642, 0x6d8085, 0x268bd2, 0x93a1a1, 0x8d9d9e, 0x4fa1db, 0x6d8085, 0x91a31a, 0xbc951a, 0xe87a78, 0x4fa1db, 0x002b36]),
    p("solarized-light", "Solarized Light", false, [0xfdf6e3, 0xeee8d5, 0x748688, 0x0e6aa8, 0x546a70, 0x576a71, 0x0e6aa8, 0x768587, 0x5f6d00, 0x826200, 0xc22c29, 0x0e6aa8, 0xfdf6e3]),
    p("one-dark", "One Dark", true, [0x282c34, 0x21252b, 0x717781, 0x61afef, 0xabb2bf, 0x8d95a3, 0x61afef, 0x717782, 0x98c379, 0xe5c07b, 0xe2737c, 0x61afef, 0x282c34]),
    p("kanagawa", "Kanagawa", true, [0x1f1f28, 0x16161d, 0x6e6d6d, 0x7e9cd8, 0xdcd7ba, 0xa6a69c, 0x7e9cd8, 0x727169, 0x98bb6c, 0xe6c384, 0xe46876, 0x7e9cd8, 0x1f1f28]),
    p("everforest", "Everforest", true, [0x2d353b, 0x232a2e, 0x7f8079, 0xa7c080, 0xd3c6aa, 0x9da9a0, 0xa7c080, 0x859289, 0xa7c080, 0xdbbc7f, 0xe68183, 0xa7c080, 0x2d353b]),
];

impl Palette {
    pub fn named(name: &str) -> &'static Palette {
        let name = name.trim().to_ascii_lowercase();
        ALL.iter().find(|p| p.name == name).unwrap_or(&ALL[0])
    }

    pub fn next(&self) -> &'static Palette {
        let at = ALL.iter().position(|p| p.name == self.name).unwrap_or(0);
        &ALL[(at + 1) % ALL.len()]
    }

    pub fn text_pairs(&self) -> Vec<(&'static str, u32, u32)> {
        let mut pairs = Vec::new();
        for ground in [self.bg, self.bg_alt] {
            for (role, color) in [
                ("fg", self.fg),
                ("fg_dim", self.fg_dim),
                ("accent", self.accent),
                ("ok", self.ok),
                ("warn", self.warn),
                ("err", self.err),
            ] {
                pairs.push((role, color, ground));
            }
        }
        pairs.push(("selection_fg", self.selection_fg, self.selection_bg));
        pairs.push(("bg on accent", self.bg, self.accent));
        pairs
    }

    pub fn worst_text_contrast(&self) -> f32 {
        self.text_pairs()
            .into_iter()
            .map(|(_, fg, bg)| contrast(fg, bg))
            .fold(f32::MAX, f32::min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_reads_at_4_5_to_1() {
        for palette in &ALL {
            for (role, fg, bg) in palette.text_pairs() {
                let ratio = contrast(fg, bg);
                assert!(
                    ratio >= 4.5,
                    "{} {role} {fg:06x} on {bg:06x}: {ratio:.2}",
                    palette.name
                );
            }
            for ground in [palette.bg, palette.bg_alt] {
                for (role, color) in [
                    ("focus", palette.border_focus),
                    ("border", palette.border),
                    ("dim", palette.accent_dim),
                ] {
                    assert!(
                        contrast(color, ground) >= 3.0,
                        "{} {role} on {ground:06x}",
                        palette.name
                    );
                }
            }
        }
    }

    #[test]
    fn warn_is_yellow_never_orange() {
        for palette in &ALL {
            let [r, g, b] = [16, 8, 0].map(|s| ((palette.warn >> s) & 255) as f32);
            let (max, min) = (r.max(g).max(b), r.min(g).min(b));
            if max - min < 24. {
                continue;
            }
            let hue = if max == r {
                60. * ((g - b) / (max - min)).rem_euclid(6.)
            } else if max == g {
                60. * ((b - r) / (max - min) + 2.)
            } else {
                60. * ((r - g) / (max - min) + 4.)
            };
            assert!(
                !(10.0..33.0).contains(&hue),
                "{} warn hue {hue:.0}",
                palette.name
            );
        }
    }

    #[test]
    fn every_role_is_set_and_names_are_unique() {
        for (i, palette) in ALL.iter().enumerate() {
            for role in [
                palette.bg,
                palette.bg_alt,
                palette.border,
                palette.border_focus,
                palette.fg,
                palette.fg_dim,
                palette.accent,
                palette.accent_dim,
                palette.ok,
                palette.warn,
                palette.err,
                palette.selection_bg,
                palette.selection_fg,
            ] {
                assert!(role <= 0xffffff);
            }
            assert_ne!(palette.bg, palette.fg, "{}", palette.name);
            assert_ne!(palette.bg, palette.bg_alt, "{}", palette.name);
            assert!(!palette.label.is_empty());
            assert!(ALL.iter().skip(i + 1).all(|o| o.name != palette.name));
        }
    }

    #[test]
    fn grayscale_is_neutral_and_the_default() {
        let g = Palette::named(DEFAULT);
        assert_eq!(g.name, "grayscale");
        for role in [
            g.bg,
            g.bg_alt,
            g.border,
            g.border_focus,
            g.fg,
            g.fg_dim,
            g.accent,
            g.accent_dim,
            g.ok,
            g.warn,
            g.err,
            g.selection_bg,
            g.selection_fg,
        ] {
            let [r, gr, b] = [16, 8, 0].map(|s| (role >> s) & 255);
            assert!(r == gr && gr == b, "{role:06x} is not a gray");
        }
    }

    #[test]
    fn the_name_round_trips_and_unknown_falls_back() {
        for palette in &ALL {
            assert_eq!(Palette::named(palette.name), palette);
            let settings = crate::backend::Settings {
                palette: palette.name.to_string(),
                ..Default::default()
            };
            let json = serde_json::to_string(&settings).unwrap();
            let back = crate::backend::Settings::parse(&json);
            assert_eq!(Palette::named(&back.palette), palette);
        }
        assert_eq!(Palette::named("perforizon-orange").name, "grayscale");
        assert_eq!(Palette::named("").name, "grayscale");
        assert_eq!(crate::backend::Settings::parse("{}").palette, "");
    }

    #[test]
    fn cycling_visits_every_palette_once() {
        let mut seen = vec![ALL[0].name];
        let mut at = ALL[0].next();
        while at.name != ALL[0].name {
            seen.push(at.name);
            at = at.next();
        }
        assert_eq!(seen.len(), ALL.len());
    }
}
