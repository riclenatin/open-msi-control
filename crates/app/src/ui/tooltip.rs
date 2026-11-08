//! Hover explanations: an (i) glyph that opens a short tooltip.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, Context, Div, ElementId, IntoElement, Render, SharedString, Stateful,
    Window, div, prelude::*, px,
};

use crate::assets::Icon;
use crate::motion::EASE_OUT;
use crate::theme::{radius, theme};
use crate::ui::icon;

struct Tip {
    text: SharedString,
}

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        div()
            .max_w(px(280.))
            .px(px(10.))
            .py(px(8.))
            .rounded(radius::control())
            .bg(t.raised)
            .shadow_lg()
            .text_size(px(12.))
            .line_height(px(17.))
            .text_color(t.text)
            .child(self.text.clone())
            // Tooltips: 140 ms ease-out fade (Emil: 125–200 ms).
            .with_animation(
                "tip-in",
                Animation::new(Duration::from_millis(140)).with_easing(|x| EASE_OUT.ease(x)),
                |el, t| el.opacity(t),
            )
    }
}

/// (i) glyph that explains `text` on hover.
pub fn info(id: impl Into<ElementId>, text: &'static str) -> Stateful<Div> {
    let t = theme();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(18.))
        .rounded_full()
        .hover(|s| s.bg(t.hover))
        .child(icon(Icon::Info, 13., t.text_faint))
        .tooltip(move |_, cx| cx.new(|_| Tip { text: text.into() }).into())
}
