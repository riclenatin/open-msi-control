//! Borderless surfaces (tuffui): a `card` per section, `inset` for grouped rows inside it.
//! Layers differ only by lightness; there are no outlines.

use gpui::{AnyElement, Div, FontWeight, Rgba, Svg, div, prelude::*, px, svg};

use crate::assets::Icon;
use crate::theme::{radius, theme};

pub fn icon(icon: Icon, size: f32, color: Rgba) -> Svg {
    svg().path(icon.path()).size(px(size)).flex_none().text_color(color)
}

/// One section: a single surface with an inline title row and optional right-hand content.
pub fn card(i: Icon, title: &'static str, aside: Option<AnyElement>) -> Div {
    let t = theme();
    div().flex().flex_col().gap(px(10.)).p(px(12.)).rounded(radius::card()).bg(t.card).child(
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(24.))
            .child(icon(i, 15., t.text_muted))
            .child(
                div()
                    .flex_1()
                    .text_size(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .truncate()
                    .child(title),
            )
            .children(aside),
    )
}

/// Grouped rows inside a card.
pub fn inset() -> Div {
    div().flex().flex_col().rounded(radius::control()).bg(theme().inset)
}
