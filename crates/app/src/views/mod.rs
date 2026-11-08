//! Screens and cards. Each file adds `render_*` methods to [`MsiApp`].

mod battery;
mod fans;
mod header;
mod mode;
mod system;
mod telemetry;

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, Rgba, SharedString, Window, div, linear_color_stop,
    linear_gradient, prelude::*, px,
};

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::motion::{self, EASE_OUT};
use crate::theme::{radius, theme};
use crate::ui::{Tone, icon, pill};

/// Fades a card in on first appearance, staggered by `index` (rare event, so it may animate).
fn enter(el: impl IntoElement + 'static, index: usize) -> AnyElement {
    let delay = motion::STAGGER * u32::try_from(index).unwrap_or(0);
    div()
        .child(el)
        .with_animations(
            ("enter", index),
            vec![
                Animation::new(delay.max(Duration::from_millis(1))),
                Animation::new(motion::ENTER + Duration::from_millis(60)).with_easing(|t| EASE_OUT.ease(t)),
            ],
            |el, phase, t| if phase == 0 { el.opacity(0.) } else { el.opacity(t) },
        )
        .into_any_element()
}

impl MsiApp {
    pub(crate) fn render_dashboard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let error = self.error.clone().map(|e| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .rounded(radius::control())
                .bg(t.danger_dim)
                .text_size(px(13.))
                .text_color(t.text)
                .child(icon(Icon::Warning, 15., t.danger))
                .child(e)
        });

        let scroll = div().id("body").size_full().overflow_y_scroll().child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .px(px(12.))
                .pt(px(4.))
                .pb(px(14.))
                .children(error)
                .child(enter(self.render_telemetry(window, cx), 0))
                .child(enter(self.render_mode(window, cx), 1))
                .child(enter(self.render_fans(window, cx), 2))
                .child(enter(self.render_battery(window, cx), 3))
                .child(enter(self.render_system(window, cx), 4))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .px(px(4.))
                        .text_size(px(12.))
                        .text_color(t.text_faint)
                        .child(icon(Icon::Info, 13., t.text_faint))
                        .child(div().flex_1().min_w_0().child(self.tr().footer)),
                ),
        );

        // Content scrolling under the header fades out instead of being cut at a hard edge.
        let fade_top = Rgba { a: 0.0, ..t.background };
        div().relative().flex_1().min_h_0().child(scroll).child(
            div().absolute().top_0().left_0().right_0().h(px(14.)).bg(linear_gradient(
                180.,
                linear_color_stop(t.background, 0.),
                linear_color_stop(fade_top, 1.),
            )),
        )
    }

    pub(crate) fn render_connecting(&self) -> impl IntoElement + use<> {
        let t = theme();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .child(pill(Tone::Neutral, self.tr().connecting))
            .child(div().text_size(px(13.)).text_color(t.text_muted).child(self.tr().connecting_hint))
    }

    pub(crate) fn render_failure(&self, error: SharedString) -> impl IntoElement + use<> {
        let t = theme();
        div().flex_1().flex().items_center().justify_center().p(px(16.)).child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .p(px(20.))
                .rounded(radius::card())
                .bg(t.card)
                .child(icon(Icon::Warning, 26., t.danger))
                .child(div().text_size(px(15.)).child(self.tr().no_access_title))
                .child(div().text_size(px(13.)).text_color(t.text_muted).text_center().child(error))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(t.text_muted)
                        .text_center()
                        .child(self.tr().no_access_hint),
                ),
        )
    }
}
