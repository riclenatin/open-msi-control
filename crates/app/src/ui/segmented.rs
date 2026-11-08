//! Radio-style segmented control with a sliding active pill.
//!
//! Every segment has the same width, so the pill (one segment wide) always lines up with its
//! label. Content-sized controls measure the longest label; stretched controls split the width
//! the parent gives them. Pointer changes slide the pill (animated by the caller through
//! [`crate::motion::Transitions`]); arrow keys change the value instantly.

use gpui::{
    Div, ElementId, FontWeight, KeyDownEvent, Pixels, SharedString, Stateful, TextRun, Window, div, font,
    prelude::*, px, relative, transparent_black,
};

use crate::theme::{FONT_SANS, theme};

/// `on_select(index, from_keyboard)`.
pub type OnSelect = crate::ui::Handler<(usize, bool)>;

const TEXT: f32 = 13.;
const PAD_X: f32 = 10.;
const SEGMENT_H: f32 = 26.;
/// Container padding + focus border, per side.
const INSET: f32 = 3.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    /// As wide as `n × (longest label + padding)`.
    Content,
    /// Fill the parent; segments split it equally.
    Fill,
}

fn segment_width(window: &mut Window, options: &[SharedString]) -> Pixels {
    let mut f = font(FONT_SANS);
    f.weight = FontWeight::MEDIUM;
    let widest = options
        .iter()
        .map(|label| {
            let run = TextRun {
                len: label.len(),
                font: f.clone(),
                color: theme().text.into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window.text_system().layout_line(label, px(TEXT), &[run], None).width
        })
        .fold(px(0.), Pixels::max);
    (widest + px(PAD_X * 2.)).ceil()
}

/// `pill_at` is the animated pill position in segments (0.0 = first option).
#[allow(clippy::too_many_arguments, reason = "plain view-model parameters")]
pub fn segmented(
    id: impl Into<ElementId>,
    options: &[SharedString],
    selected: Option<usize>,
    pill_at: f32,
    width: Width,
    ring: bool,
    window: &mut Window,
    on_select: OnSelect,
) -> Stateful<Div> {
    let t = theme();
    let n = options.len().max(1);
    let seg = segment_width(window, options);

    let mut row = div().relative().flex().h_full();
    row = match width {
        Width::Content => row.w(seg * n as f32),
        Width::Fill => row.w_full(),
    };
    if selected.is_some() {
        let pill = div().absolute().top_0().bottom_0().rounded_full().bg(t.raised);
        row = row.child(match width {
            Width::Content => pill.left(seg * pill_at).w(seg),
            Width::Fill => pill.left(relative(pill_at / n as f32)).w(relative(1.0 / n as f32)),
        });
    }
    for (i, label) in options.iter().enumerate() {
        let on_select = on_select.clone();
        let active = selected == Some(i);
        let cell = div()
            .id(i)
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .h_full()
            .rounded_full()
            .cursor_pointer()
            .text_size(px(TEXT))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if active { t.text } else { t.text_muted })
            .when(!active, |d| d.hover(|s| s.text_color(t.text)))
            .child(label.clone())
            .on_click(move |e, window, cx| on_select((i, e.is_keyboard()), window, cx));
        row = row.child(match width {
            Width::Content => cell.flex_none().w(seg),
            Width::Fill => cell.flex_1().min_w(seg),
        });
    }

    let on_key = on_select;
    div()
        .id(id.into())
        .tab_index(0)
        .flex_none()
        .h(px(SEGMENT_H) + px(INSET * 2.))
        .p(px(INSET - 2.))
        .rounded_full()
        .bg(t.inset)
        .border_2()
        .border_color(transparent_black())
        .when(ring, |d| d.focus(|s| s.border_color(t.focus)))
        .when(width == Width::Fill, |d| d.w_full())
        .on_key_down(move |e: &KeyDownEvent, window, cx| {
            let cur = selected.unwrap_or(0);
            let next = match e.keystroke.key.as_str() {
                "left" | "up" => cur.checked_sub(1),
                "right" | "down" => (cur + 1 < n).then_some(cur + 1),
                "home" => Some(0),
                "end" => Some(n - 1),
                _ => return,
            };
            cx.stop_propagation();
            if let Some(next) = next.filter(|&i| Some(i) != selected) {
                on_key((next, true), window, cx);
            }
        })
        .child(row)
}
