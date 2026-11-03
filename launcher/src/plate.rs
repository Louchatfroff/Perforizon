use crate::theme::night::Night;
use gpui::{div, prelude::*, px, rgba, Div, Rgba};

pub fn alpha(c: u32, a: f32) -> Rgba {
    rgba(c << 8 | (a * 255.).round() as u32)
}

pub fn plate(n: &Night) -> Div {
    cut(
        0.,
        0,
        Some(alpha(n.card, n.card_alpha)),
        Some((alpha(n.stroke, n.stroke_alpha), 1.)),
    )
}

pub fn cut(_step: f32, _steps: usize, fill: Option<Rgba>, stroke: Option<(Rgba, f32)>) -> Div {
    let d = div().relative().rounded(px(0.));
    let d = match fill {
        Some(c) => d.bg(c),
        None => d,
    };
    match stroke {
        Some((c, _)) => d.border_1().border_color(c),
        None => d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_packs_the_colour_and_its_opacity() {
        let c = alpha(0x336699, 1.);
        assert_eq!((c.r * 255.).round() as u32, 0x33);
        assert_eq!((c.a * 255.).round() as u32, 0xff);
        assert_eq!((alpha(0, 0.5).a * 255.).round() as u32, 128);
    }
}
