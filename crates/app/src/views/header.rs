//! Window header: dithered horizon art, custom window chrome, model and live gauges.

use gpui::{
    Context, FontWeight, ObjectFit, Rgba, Window, WindowControlArea, div, img, linear_color_stop,
    linear_gradient, prelude::*, px,
};

use crate::app::{Connection, MsiApp};
use crate::assets::Icon;
use crate::charts::tick_gauge;
use crate::motion::{self, Spec};
use crate::theme::{FONT_MONO, theme};
use crate::ui::{Tone, icon, mono, pill};

pub const HEIGHT: f32 = 128.;
const GAUGE_W: f32 = 70.;
/// A 240° arc is 1.5 radii tall.
const GAUGE_H: f32 = 56.;

fn window_button(
    id: &'static str,
    glyph: Icon,
    area: WindowControlArea,
    danger: bool,
) -> impl IntoElement + use<> {
    let t = theme();
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(42.))
        .h(px(32.))
        .window_control_area(area)
        .hover(move |s| s.bg(if danger { t.danger } else { t.hover }))
        .child(icon(glyph, 13., t.text))
}

/// Temperature in the middle of the arc, sensor name between its feet.
fn gauge(label: &'static str, value: Option<f32>, raw: Option<u8>) -> impl IntoElement {
    let t = theme();
    div()
        .relative()
        .w(px(GAUGE_W))
        .h(px(GAUGE_H))
        .child(tick_gauge(value, (30., 100.), 75., 90.))
        .child(div().absolute().top(px(20.)).left_0().right_0().flex().justify_center().child(
            mono(raw.map_or("—".to_owned(), |v| format!("{v}°"))).text_size(px(17.)).text_color(t.text),
        ))
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(mono(label).text_size(px(10.)).text_color(t.text_muted)),
        )
}

impl MsiApp {
    pub(crate) fn render_header(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let tr = self.tr();
        let scrim = Rgba { a: 0.0, ..t.background };

        // Status only matters while it is not "connected and reading".
        let status = match &self.conn {
            Connection::Ready(_) if self.snap.is_some() => None,
            Connection::Failed(_) => Some(pill(Tone::Danger, tr.no_access)),
            _ => Some(pill(Tone::Neutral, tr.connecting)),
        };
        let firmware = self.info.as_ref().map_or_else(
            || "MSI_ACPI".to_owned(),
            |i| format!("EC {} · WMI {}.{}", i.ec_firmware, i.wmi_version.0, i.wmi_version.1),
        );

        let gauges = self.snap.clone().map(|s| {
            let mut eased = |key, v: Option<u8>| {
                v.map(|v| self.transitions.animate(("gauge", key), f32::from(v), Spec::value(motion::VALUE)))
            };
            let (cpu, gpu) = (eased(0, s.cpu.temp_c), eased(1, s.gpu.temp_c));
            div()
                .flex()
                .gap(px(2.))
                .px(px(6.))
                .py(px(4.))
                .rounded(px(14.))
                .bg(Rgba { a: 0.72, ..t.background })
                .child(gauge("CPU", cpu, s.cpu.temp_c))
                .child(gauge("GPU", gpu, s.gpu.temp_c))
        });

        div()
            .relative()
            .flex_none()
            .h(px(HEIGHT))
            .w_full()
            .overflow_hidden()
            .child(
                img(self.dither.clone()).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover),
            )
            .child(div().absolute().bottom_0().left_0().w_full().h(px(40.)).bg(linear_gradient(
                180.,
                linear_color_stop(scrim, 0.),
                linear_color_stop(t.background, 1.),
            )))
            // Title bar: draggable, with window buttons.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id("title-drag")
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .pl(px(14.))
                            .window_control_area(WindowControlArea::Drag)
                            .child(icon(Icon::Fan, 13., t.accent))
                            .child(
                                div()
                                    .font_family(FONT_MONO)
                                    .text_size(px(12.))
                                    .text_color(t.text_muted)
                                    .child("open-msi-control"),
                            ),
                    )
                    .child(window_button("win-min", Icon::Minimize, WindowControlArea::Min, false))
                    .child(window_button("win-close", Icon::Close, WindowControlArea::Close, true)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(18.))
                    .right(px(12.))
                    .bottom(px(12.))
                    .flex()
                    .items_end()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .min_w_0()
                            .children(status.map(|p| div().flex().child(p)))
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.text)
                                    .truncate()
                                    .child(
                                        self.model.clone().unwrap_or_else(|| tr.model_fallback.to_owned()),
                                    ),
                            )
                            .child(mono(firmware).text_size(px(11.)).text_color(t.text_muted).truncate()),
                    )
                    .children(gauges),
            )
    }
}
