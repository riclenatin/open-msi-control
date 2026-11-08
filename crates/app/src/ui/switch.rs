use gpui::{Div, ElementId, SharedString, Stateful, div, prelude::*, px, transparent_black};

use crate::assets::Icon;
use crate::theme::theme;
use crate::ui::icon;

const TRACK_W: f32 = 40.;
const TRACK_H: f32 = 24.;
const THUMB: f32 = 18.;

/// `on_toggle(new_value, from_keyboard)`.
pub type OnToggle = crate::ui::Handler<(bool, bool)>;

/// Full-width settings row; the whole row is the hit target.
///
/// `thumb_at` is the animated thumb position, 0.0 = off, 1.0 = on.
#[allow(clippy::too_many_arguments, reason = "plain view-model parameters")]
pub fn switch_row(
    id: impl Into<ElementId>,
    leading: Icon,
    label: impl Into<SharedString>,
    description: Option<SharedString>,
    on: bool,
    thumb_at: f32,
    ring: bool,
    on_toggle: OnToggle,
) -> Stateful<Div> {
    let t = theme();
    let travel = TRACK_W - THUMB - 6.;
    div()
        .id(id)
        .tab_index(0)
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(7.))
        .rounded(px(12.))
        .border_2()
        .border_color(transparent_black())
        .when(ring, |d| d.focus(|s| s.border_color(t.focus)))
        .cursor_pointer()
        .hover(|s| s.bg(t.hover))
        .child(icon(leading, 15., if on { t.accent } else { t.text_muted }))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(div().text_size(px(13.)).text_color(t.text).child(label.into()))
                .children(description.map(|d| div().text_size(px(11.)).text_color(t.text_muted).child(d))),
        )
        .child(
            div()
                .relative()
                .flex_none()
                .w(px(TRACK_W))
                .h(px(TRACK_H))
                .rounded_full()
                .bg(if on { t.accent } else { t.raised })
                .child(
                    div()
                        .absolute()
                        .top(px((TRACK_H - THUMB) / 2.))
                        .left(px(3. + travel * thumb_at))
                        .size(px(THUMB))
                        .rounded_full()
                        .bg(if on { t.on_accent } else { t.text_muted }),
                ),
        )
        .on_click(move |e, window, cx| on_toggle((!on, e.is_keyboard()), window, cx))
}
