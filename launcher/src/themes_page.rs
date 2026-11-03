use super::*;
use horizon_launcher::palette::{self, Palette};

impl Launcher {
    pub(super) fn settings_theme(&self) -> Theme {
        Theme::named(&self.settings.palette)
    }

    pub(super) fn set_palette(&mut self, palette: &'static Palette, window: &mut Window) {
        self.settings.palette = palette.name.to_string();
        self.settings.save();
        titlebar::apply_appearance(
            window,
            if palette.dark {
                backend::Appearance::Dark
            } else {
                backend::Appearance::Light
            },
        );
        self.theme = self.settings_theme();
    }

    pub(super) fn cycle_palette(&mut self, window: &mut Window) {
        let next = Palette::named(&self.settings.palette).next();
        self.set_palette(next, window);
    }

    pub(super) fn palette_row(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let mut list = div().flex().flex_wrap().gap(px(8.)).max_w(px(560.));
        for p in palette::ALL.iter() {
            let chosen = self.settings.palette_name() == p.name;
            list = list.child(
                keyboard(
                    div().id(SharedString::from(format!("palette-{}", p.name))),
                    &t,
                )
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .cursor_pointer()
                .border_1()
                .border_color(if chosen { t.focus } else { t.outline })
                .when(chosen, |d| d.bg(t.sel_bg).text_color(t.sel_fg))
                .when(!chosen, |d| {
                    d.text_color(t.ink).hover(move |s| s.bg(t.raised))
                })
                .child(format!("{}{}", if chosen { "› " } else { "" }, p.label))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.set_palette(p, window);
                    cx.notify();
                })),
            );
        }
        index_row(
            0,
            tr("Palette"),
            Some(
                div()
                    .child(tr("Grayscale by default. ^p cycles through the palettes."))
                    .into_any_element(),
            ),
            Some(list.into_any_element()),
            &t,
        )
    }
}
