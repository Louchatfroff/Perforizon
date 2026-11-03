//! The terminal look's small parts: glyph icons, bordered panels, badges, tabs and the slots that
//! hold icons downloaded at run time.
use crate::theme::{self, Theme};
use gpui::{
    div, img, prelude::*, px, App, ClickEvent, Div, FontWeight, Hsla, ImageSource, ObjectFit,
    SharedString, Window,
};
use std::borrow::Cow;

pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, _path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(None)
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

pub fn icon(name: &'static str, size: f32, color: Hsla) -> Div {
    crate::tui::glyph_text(name, size, color)
}

pub fn pixel(size: f32, weight: FontWeight) -> Div {
    div()
        .font_family(theme::PIXEL)
        .font_weight(weight)
        .text_size(px(size))
        .line_height(px((size * 1.4).round()))
}

pub fn panel(t: &Theme) -> Div {
    div()
        .relative()
        .bg(t.panel)
        .border_1()
        .border_color(t.outline)
        .rounded(px(0.))
}

pub fn badge(label: impl Into<SharedString>, color: Hsla, t: &Theme) -> Div {
    let label: SharedString = label.into();
    let label = SharedString::from(crate::i18n::tr(&label).to_string());
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(22.))
        .px(px(8.))
        .border_1()
        .border_color(t.outline)
        .rounded(px(0.))
        .child(div().text_color(color).child("●"))
        .child(
            pixel(12., FontWeight::NORMAL)
                .text_color(color)
                .child(label),
        )
}

pub fn tabs<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &[(T, &'static str, &'static str)],
    current: T,
    t: &Theme,
    on_pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> Div {
    let t = *t;
    div()
        .flex()
        .items_end()
        .gap(px(4.))
        .children(options.iter().map(|&(value, label, _)| {
            let open = current == value;
            let pick = on_pick.clone();
            let tab = div()
                .id(SharedString::from(format!("{id}-{label}")))
                .flex()
                .items_center()
                .h(px(28.))
                .px(px(12.))
                .cursor_pointer()
                .font_family(theme::MONO)
                .text_size(px(14.))
                .when(open, |d| d.bg(t.sel_bg).text_color(t.sel_fg))
                .when(!open, |d| {
                    d.text_color(t.muted)
                        .hover(move |s| s.bg(t.raised).text_color(t.ink))
                })
                .child(crate::i18n::tr(label))
                .on_click(move |_: &ClickEvent, window, cx| pick(value, window, cx));
            crate::keyboard(tab, &t)
        }))
}

/// A square slot for an icon: the picture at `source` when there is one, else the first letter of
/// `name` on a raised block.
pub fn slot(source: Option<ImageSource>, name: &str, size: f32, t: &Theme) -> Div {
    let frame = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .overflow_hidden()
        .border_1()
        .border_color(t.outline)
        .bg(t.well);
    match source {
        Some(source) => frame.child(img(source).size(px(size - 2.)).object_fit(ObjectFit::Cover)),
        None => frame.bg(t.raised).child(
            pixel((size * 0.42).round(), FontWeight::NORMAL)
                .text_color(t.muted)
                .child(
                    name.chars()
                        .find(|c| c.is_alphanumeric())
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string(),
                ),
        ),
    }
}
