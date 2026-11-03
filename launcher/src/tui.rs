use crate::theme::{self, Theme};
use gpui::{div, prelude::*, px, Div, FontWeight, Hsla, SharedString};

pub const WORDMARK: &str = "P E R F O R I Z O N";
pub const BAR_CELLS: usize = 32;
pub const HINTS: [(&str, &str); 5] = [
    ("tab", "next"),
    ("shift-tab", "previous"),
    ("enter", "select"),
    ("esc", "back"),
    ("^p", "palette"),
];

pub fn block_bar(fraction: f32, cells: usize) -> String {
    let filled = ((fraction.clamp(0., 1.) * cells as f32).round() as usize).min(cells);
    format!("{}{}", "█".repeat(filled), "░".repeat(cells - filled))
}

pub fn shuttle_bar(phase: f32, cells: usize, width: usize) -> String {
    let start = ((phase.rem_euclid(1.) * (cells + width) as f32) as usize).saturating_sub(width);
    (0..cells)
        .map(|i| {
            if i >= start && i < start + width {
                '█'
            } else {
                '░'
            }
        })
        .collect()
}

#[cfg(test)]
pub fn spaced(word: &str) -> String {
    word.chars().map(String::from).collect::<Vec<_>>().join(" ")
}

pub fn glyph(name: &str) -> &'static str {
    match name {
        "archive" => "_",
        "arrow-left" => "←",
        "book-open" => "§",
        "box" => "¤",
        "bug" => "!",
        "check" => "√",
        "chevron-down" => "v",
        "chevron-right" => "›",
        "clock" => "…",
        "close" => "×",
        "download" => "↓",
        "external-link" => "→",
        "folder" => "¦",
        "gamepad" => "◊",
        "globe" => "○",
        "heart" => "&",
        "home" => "^",
        "image" => "%",
        "info-box" | "info" => "▲",
        "message" => "@",
        "monitor" => "|",
        "notes" => "¶",
        "package" => "#",
        "play" => "▶",
        "plus" => "+",
        "reload" => "≈",
        "search" => "/",
        "server" => "$",
        "sliders" => "=",
        "star" => "*",
        "terminal" => "›",
        "trash" => "x",
        "user" => "●",
        "zap" => "~",
        _ => "•",
    }
}

pub fn glyph_text(name: &str, size: f32, color: Hsla) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .font_family(theme::MONO)
        .text_size(px(size))
        .line_height(px(size))
        .text_color(color)
        .child(glyph(name))
}

pub fn panel(title: impl Into<SharedString>, focused: bool, ground: Hsla, t: &Theme) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .border_1()
        .border_color(if focused { t.focus } else { t.outline })
        .rounded(px(0.))
        .child(
            div()
                .absolute()
                .top(px(-9.))
                .left(px(12.))
                .px(px(6.))
                .h(px(16.))
                .bg(ground)
                .font_family(theme::MONO)
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(if focused { t.ink } else { t.muted })
                .child(title.into()),
        )
}

pub fn rule(title: impl Into<SharedString>, t: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .font_family(theme::MONO)
        .text_size(px(13.))
        .line_height(px(20.))
        .child(div().flex_none().w(px(16.)).h(px(1.)).bg(t.outline))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::NORMAL)
                .text_color(t.ink)
                .child(title.into()),
        )
        .child(div().flex_1().h(px(1.)).bg(t.outline))
}

pub fn hatched(label: impl Into<SharedString>, t: &Theme) -> Div {
    let line = "/".repeat(900);
    div()
        .relative()
        .w_full()
        .h(px(96.))
        .overflow_hidden()
        .font_family(theme::MONO)
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(t.outline)
        .children((0..6).map(|_| div().whitespace_nowrap().child(line.clone())))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .px(px(8.))
                        .bg(t.page)
                        .text_color(t.muted)
                        .child(label.into()),
                ),
        )
}

pub fn footer(extra: Option<(&str, &str)>, t: &Theme) -> Div {
    let hint = |key: &str, what: &str| {
        div()
            .flex()
            .flex_none()
            .gap(px(6.))
            .child(div().text_color(t.ink).child(key.to_string()))
            .child(
                div()
                    .text_color(t.muted)
                    .child(crate::i18n::tr(what).to_string()),
            )
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(16.))
        .h(px(theme::FOOTER))
        .px(px(theme::PAGE_GUTTER - 8.))
        .border_t_1()
        .border_color(t.outline)
        .bg(t.page)
        .font_family(theme::MONO)
        .text_size(px(12.))
        .line_height(px(16.))
        .children(HINTS.iter().map(|(k, w)| hint(k, w)))
        .children(extra.map(|(k, w)| hint(k, w)))
        .child(
            div()
                .ml_auto()
                .flex_none()
                .text_color(t.muted)
                .child(t.palette.label),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_fills_in_whole_cells() {
        assert_eq!(block_bar(0., 4), "░░░░");
        assert_eq!(block_bar(0.5, 4), "██░░");
        assert_eq!(block_bar(1., 4), "████");
        assert_eq!(block_bar(7., 4), "████");
        assert_eq!(block_bar(-1., 4), "░░░░");
        assert_eq!(block_bar(0.3, BAR_CELLS).chars().count(), BAR_CELLS);
    }

    #[test]
    fn the_shuttle_stays_inside_the_bar() {
        for step in 0..=20 {
            let bar = shuttle_bar(step as f32 / 20., 10, 3);
            assert_eq!(bar.chars().count(), 10);
            assert!(bar.chars().filter(|&c| c == '█').count() <= 3);
        }
        assert_eq!(
            shuttle_bar(0.5, 10, 3)
                .chars()
                .filter(|&c| c == '█')
                .count(),
            3
        );
    }

    #[test]
    fn the_wordmark_is_spaced_out() {
        assert_eq!(spaced("PERFORIZON"), WORDMARK);
        assert_eq!(spaced("ab"), "a b");
    }

    fn font_has(c: char) -> bool {
        let face = ttf_parser::Face::parse(horizon_theme::fonts::GEIST_MONO, 0).unwrap();
        face.glyph_index(c).is_some()
    }

    #[test]
    fn every_icon_name_has_its_own_glyph() {
        for name in horizon_theme::icons::ALL.iter().map(|(n, _)| *n) {
            assert_ne!(glyph(name), "•", "{name}");
        }
    }

    #[test]
    fn every_glyph_and_marker_is_in_the_bundled_font() {
        let mut shown: Vec<&str> = horizon_theme::icons::ALL
            .iter()
            .map(|(n, _)| glyph(n))
            .collect();
        shown.extend(["›", "√", "█", "░", "…", "●", "×", "·", "/"]);
        for g in shown {
            for c in g.chars() {
                assert!(font_has(c), "U+{:04X} {c} is not in Geist Mono", c as u32);
            }
        }
    }
}
