use gpui::{Div, div, prelude::*, px};

use crate::assets::Icon;
use crate::theme::theme;
use crate::ui::{icon, info};

/// Label row with an optional (i) explanation and a trailing control (segmented, value…).
pub fn control_row(
    glyph: Icon,
    active: bool,
    label: &'static str,
    explain: Option<&'static str>,
    control: impl IntoElement,
) -> Div {
    let t = theme();
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .pl(px(12.))
        .child(icon(glyph, 15., if active { t.accent } else { t.text_muted }))
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .gap(px(4.))
                .child(div().text_size(px(13.)).child(label))
                .children(explain.map(|text| info(label, text))),
        )
        .child(control)
}
