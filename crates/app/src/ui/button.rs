use gpui::{
    App, ClickEvent, ElementId, FontWeight, IntoElement, RenderOnce, SharedString, Window, div,
    linear_color_stop, linear_gradient, prelude::*, px, transparent_black,
};

use crate::assets::Icon;
use crate::theme::theme;
use crate::ui::icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Silver gradient, near-black label: the one action that commits.
    Primary,
    /// Dark gradient: alternatives such as "Reset".
    Secondary,
}

type OnClick = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    icon: Option<Icon>,
    variant: Variant,
    ring: bool,
    on_click: Option<OnClick>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, variant: Variant) -> Self {
        Self { id: id.into(), label: label.into(), icon: None, variant, ring: false, on_click: None }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Show the focus ring (keyboard navigation active).
    pub fn ring(mut self, ring: bool) -> Self {
        self.ring = ring;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = theme();
        let (top, bottom, label, hover_top) = match self.variant {
            Variant::Primary => (t.silver_top, t.silver_bottom, t.on_silver, t.silver_top),
            Variant::Secondary => (t.raised, t.card, t.text, t.raised),
        };
        div()
            .id(self.id)
            .tab_index(0)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .h(px(32.))
            .px(px(14.))
            .rounded_full()
            .border_2()
            .border_color(transparent_black())
            .when(self.ring, |d| d.focus(|s| s.border_color(t.focus)))
            .bg(linear_gradient(0., linear_color_stop(bottom, 0.), linear_color_stop(top, 1.)))
            .hover(move |s| {
                s.bg(linear_gradient(0., linear_color_stop(bottom, 0.), linear_color_stop(hover_top, 0.6)))
            })
            .active(|s| s.opacity(0.85))
            .cursor_pointer()
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(label)
            .children(self.icon.map(|i| icon(i, 15., label)))
            .child(self.label)
            .when_some(self.on_click, |d, f| d.on_click(f))
    }
}
