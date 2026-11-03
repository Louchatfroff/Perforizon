use crate::theme::{self, night, Step, Theme};
use crate::{hover_fade, keyboard, track_hover};
use gpui::{div, prelude::*, px, rgb, Div, FontWeight, Hsla, SharedString, Stateful};

pub fn c(color: u32) -> Hsla {
    rgb(color).into()
}

pub fn text(step: Step, color: Hsla) -> Div {
    div()
        .font_family(theme::MONO)
        .font_weight(FontWeight::NORMAL)
        .text_size(px(step.px))
        .line_height(px(step.line_px()))
        .text_color(color)
}

pub fn sidebar_plate(t: &Theme) -> Div {
    div().bg(t.side).border_r_1().border_color(t.outline)
}

fn hovered(id: &SharedString, t: &Theme) -> gpui::AnyElement {
    let color = t.raised;
    let layer = move || div().absolute().top_0().left_0().size_full().bg(color);
    hover_fade(id, layer, |d, a| d.opacity(a))
}

pub fn row(id: impl Into<SharedString>, selected_now: bool, t: &Theme) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_h(px(40.))
        .px(px(8.))
        .py(px(2.))
        .font_family(theme::MONO)
        .cursor_pointer();
    let d = if selected_now {
        d.bg(t.sel_bg).text_color(t.sel_fg)
    } else {
        d.child(hovered(&id, t))
    };
    let marker = div()
        .relative()
        .flex_none()
        .w(px(12.))
        .text_color(if selected_now { t.sel_fg } else { t.muted })
        .child(if selected_now { "›" } else { "" });
    keyboard(track_hover(d.child(marker), id), t)
}

pub fn row_text(
    name: impl Into<SharedString>,
    sub: String,
    state: Option<Hsla>,
    selected: bool,
    t: &Theme,
) -> Div {
    let (name_color, sub_color) = if selected {
        (t.sel_fg, t.sel_fg)
    } else {
        (t.ink, t.muted)
    };
    div()
        .relative()
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            text(night::UI, name_color)
                .font_weight(FontWeight::NORMAL)
                .truncate()
                .child(name.into()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .children(state.map(|s| {
                    div()
                        .flex_none()
                        .text_size(px(10.))
                        .text_color(if selected { t.sel_fg } else { s })
                        .child("●")
                }))
                .child(text(night::CAPTION, sub_color).truncate().child(sub)),
        )
}

pub fn heading(label: impl Into<SharedString>, t: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(28.))
        .px(px(8.))
        .mt(px(4.))
        .child(
            text(night::CAPTION, t.muted)
                .font_weight(FontWeight::NORMAL)
                .child(label.into()),
        )
}

pub fn tab(id: impl Into<SharedString>, label: &str, active: bool, t: &Theme) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .items_center()
        .h(px(28.))
        .px(px(12.))
        .cursor_pointer();
    let d = if active {
        d.bg(t.sel_bg)
    } else {
        d.child(hovered(&id, t))
    };
    keyboard(
        track_hover(
            d.child(
                text(night::UI, if active { t.sel_fg } else { t.muted })
                    .relative()
                    .child(label.to_string()),
            ),
            id,
        ),
        t,
    )
}

pub fn icon_button(
    id: impl Into<SharedString>,
    icon: &'static str,
    size: f32,
    active: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .border_1()
        .border_color(t.outline)
        .rounded(px(0.))
        .cursor_pointer();
    let d = if active {
        d.bg(t.sel_bg)
    } else {
        d.child(hovered(&id, t))
    };
    keyboard(
        track_hover(
            d.child(div().relative().child(crate::blocks::icon(
                icon,
                (size * 0.55).round(),
                if active { t.sel_fg } else { t.ink },
            ))),
            id,
        ),
        t,
    )
}

pub fn chip(id: impl Into<SharedString>, t: &Theme) -> Stateful<Div> {
    let id = id.into();
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .min_w_0()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .px(px(10.))
        .border_1()
        .border_color(t.outline)
        .rounded(px(0.))
        .cursor_pointer()
        .child(hovered(&id, t));
    keyboard(track_hover(d, id), t)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Main,
    Raised,
    Quiet,
}

pub fn action(id: impl Into<SharedString>, kind: Action, t: &Theme) -> Stateful<Div> {
    let id = id.into();
    let t = *t;
    let d = div()
        .id(id.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .px(px(24.))
        .font_family(theme::MONO)
        .font_weight(FontWeight::NORMAL)
        .text_size(px(night::UI.px))
        .cursor_pointer();
    let bracket = |glyph: &'static str, left: bool| {
        let b = div()
            .absolute()
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .child(glyph);
        if left {
            b.left(px(8.))
        } else {
            b.right(px(8.))
        }
    };
    let d = match kind {
        Action::Main => d
            .h(px(night::PLAY))
            .px(px(36.))
            .text_size(px(night::TITLE.px))
            .font_weight(FontWeight::NORMAL)
            .bg(t.face)
            .text_color(t.on_face)
            .hover(move |s| s.bg(t.ink))
            .child(bracket("[", true))
            .child(bracket("]", false)),
        Action::Raised => d
            .h(px(theme::CONTROL))
            .text_color(t.ink)
            .hover(move |s| s.bg(t.sel_bg).text_color(t.sel_fg))
            .child(bracket("[", true))
            .child(bracket("]", false)),
        Action::Quiet => d
            .h(px(theme::CONTROL))
            .text_color(t.muted)
            .hover(move |s| s.bg(t.raised).text_color(t.ink)),
    };
    keyboard(track_hover(d, id), &t).active(|s| s.opacity(theme::PRESSED))
}

pub fn label(text_: impl Into<SharedString>) -> Div {
    div().relative().child(text_.into())
}
