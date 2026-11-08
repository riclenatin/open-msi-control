use gpui::{Div, SharedString, div, prelude::*, px};

use crate::theme::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Neutral,
    Warning,
    Danger,
}

/// Tinted status pill with a leading dot.
pub fn pill(tone: Tone, label: impl Into<SharedString>) -> Div {
    let t = theme();
    let (fg, bg) = match tone {
        Tone::Accent => (t.accent, t.accent_dim),
        Tone::Neutral => (t.text_muted, t.inset),
        Tone::Warning => (t.warning, t.inset),
        Tone::Danger => (t.danger, t.danger_dim),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(24.))
        .px(px(10.))
        .rounded_full()
        .bg(bg)
        .text_size(px(12.))
        .text_color(fg)
        .child(div().size(px(6.)).rounded_full().bg(fg))
        .child(label.into())
}
