//! The launcher's terminal look: one monospace face (Geist Mono, bundled, SIL Open Font License)
//! and colours that all come from a `palette::Palette`, so a palette switch recolours everything.
use gpui::{rgb, Hsla};
#[cfg(target_os = "macos")]
use horizon_launcher::backend::Appearance;
use horizon_launcher::palette::Palette;
use horizon_theme as tokens;
use std::time::Duration;

pub use tokens::night::Night;

pub mod night {
    pub use horizon_theme::night::{Night, CAPTION, TITLE, UI};

    pub const PIXEL_CORNER: f32 = 0.;
    pub const RADIUS_CONTROL: f32 = 0.;
    pub const PLAY: f32 = 40.;
}
pub use tokens::TITLE as DISPLAY;
pub use tokens::{Step, CAPTION, HEADING, LABEL, MICRO, TITLE, UI};
pub const FONT: &str = tokens::MONO;
pub const MONO: &str = tokens::MONO;
pub const PIXEL: &str = tokens::MONO;

pub use tokens::{CONTROL_LARGE, DISABLED, FIELD, PRESSED, RISE, ROW, SEGMENT};

pub const RADIUS: f32 = 0.;
pub const RADIUS_PLATE: f32 = 0.;
pub const CONTROL: f32 = 32.;
pub const TOPBAR: f32 = 44.;
pub const FOOTER: f32 = 28.;
pub const SIDEBAR: f32 = 280.;
pub const PAGE_GUTTER: f32 = 24.;

pub const QUICK: Duration = tokens::QUICK.duration();
pub const MOVE: Duration = tokens::MOVE.duration();
pub const ENTER: Duration = tokens::ENTER.duration();
pub const STAGGER: Duration = tokens::STAGGER;

#[derive(Clone, Copy)]
pub struct Theme {
    pub page: Hsla,
    pub plate_soft: Hsla,
    pub plate: Hsla,
    pub surface: Hsla,
    pub rule: Hsla,
    pub ink: Hsla,
    pub ink_soft: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub success: Hsla,
    pub warn: Hsla,
    pub side: Hsla,
    pub panel: Hsla,
    pub raised: Hsla,
    pub well: Hsla,
    pub outline: Hsla,
    pub face: Hsla,
    pub on_face: Hsla,
    pub focus: Hsla,
    pub sel_bg: Hsla,
    pub sel_fg: Hsla,
    pub night: Night,
    pub dark: bool,
    pub palette: &'static Palette,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

#[cfg(target_os = "macos")]
pub fn pinned_dark(choice: Appearance) -> Option<bool> {
    match std::env::var("HORIZON_LAUNCHER_APPEARANCE").as_deref() {
        Ok("light") => Some(false),
        Ok("dark") => Some(true),
        _ => match choice {
            Appearance::Dark => Some(true),
            Appearance::Light => Some(false),
            Appearance::System => None,
        },
    }
}

impl Theme {
    pub fn for_palette(p: &'static Palette) -> Self {
        let night = Night {
            sky: [p.bg; 6],
            page: p.bg,
            side: p.bg_alt,
            card: p.bg,
            card_alpha: 1.,
            card_hi: p.bg_alt,
            card_hi_alpha: 1.,
            stroke: p.border,
            stroke_alpha: 1.,
            rule_alpha: 1.,
            ink: p.fg,
            ink_soft: p.fg,
            muted: p.fg_dim,
            accent: p.accent,
            accent_hi: p.accent,
            on_accent: p.bg,
            success: p.ok,
            warn: p.warn,
            danger: p.err,
            dark: p.dark,
        };
        Self {
            page: c(p.bg),
            plate_soft: c(p.bg_alt),
            plate: c(p.bg_alt),
            surface: c(p.bg),
            rule: c(p.border),
            ink: c(p.fg),
            ink_soft: c(p.fg),
            muted: c(p.fg_dim),
            accent: c(p.accent),
            on_accent: c(p.bg),
            danger: c(p.err),
            success: c(p.ok),
            warn: c(p.warn),
            side: c(p.bg_alt),
            panel: c(p.bg),
            raised: c(p.bg_alt),
            well: c(p.bg_alt),
            outline: c(p.border),
            face: c(p.accent),
            on_face: c(p.bg),
            focus: c(p.border_focus),
            sel_bg: c(p.selection_bg),
            sel_fg: c(p.selection_fg),
            night,
            dark: p.dark,
            palette: p,
        }
    }

    pub fn named(name: &str) -> Self {
        Self::for_palette(Palette::named(name))
    }
}

/// How strongly a selected suggestion in a text field is tinted with the accent.
pub const SELECTION: f32 = 0.22;

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_launcher::palette::ALL;

    #[test]
    fn a_theme_reads_only_the_palettes_roles() {
        for p in &ALL {
            let t = Theme::for_palette(p);
            assert_eq!(t.page, c(p.bg));
            assert_eq!(t.ink, c(p.fg));
            assert_eq!(t.muted, c(p.fg_dim));
            assert_eq!(t.accent, c(p.accent));
            assert_eq!(t.sel_bg, c(p.selection_bg));
            assert_eq!(t.sel_fg, c(p.selection_fg));
            assert_eq!(t.dark, p.dark);
        }
    }

    #[test]
    fn an_unknown_name_is_grayscale() {
        assert_eq!(Theme::named("nope").palette.name, "grayscale");
    }
}
