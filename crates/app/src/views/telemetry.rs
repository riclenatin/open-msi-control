use std::time::Instant;

use gpui::{Context, Div, Rgba, Window, div, prelude::*, px, relative};
use msi_acpi::FanStatus;

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::charts::{TEMP_DOMAIN, TimelineChart, bars, segment_meter, unit};
use crate::telemetry::WINDOW;
use crate::theme::theme;
use crate::ui::{card, inset, mono, tnum};

const CHART_H: f32 = 96.;
/// Fan tach ceiling used to scale sparklines.
const RPM_MAX: u32 = 6500;

fn fmt_temp(t: Option<u8>) -> String {
    t.map_or("—".to_owned(), |v| format!("{v}°"))
}

fn fan_row(name: &'static str, dashed: bool, f: &FanStatus, recent: &[u32], color: Rgba) -> Div {
    let t = theme();
    // Solid swatch for CPU, two dashes for GPU — mirrors the chart line styles.
    let swatch = || div().w(px(if dashed { 5. } else { 12. })).h(px(3.)).rounded_full().bg(color);
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(7.))
        .text_size(px(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(2.))
                .w(px(12.))
                .child(swatch())
                .when(dashed, |d| d.child(swatch())),
        )
        .child(div().w(px(30.)).text_color(t.text).child(name))
        .child(mono(fmt_temp(f.temp_c)).w(px(30.)).text_color(t.text))
        .child(div().flex_1().child(bars(recent, RPM_MAX, color)))
        .child(mono(format!("{} rpm", f.rpm)).map(tnum).w(px(66.)).text_right().text_color(t.text_muted))
        .child(segment_meter(f32::from(f.duty) / 100.0, 10, color))
        .child(mono(format!("{}%", f.duty)).w(px(30.)).text_right().text_color(t.text_muted))
}

impl MsiApp {
    pub(crate) fn render_telemetry(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let Some(s) = self.snap.clone() else { return div() };

        let tooltip = self.chart_hover.and_then(|x| {
            let age = (1.0 - x) * WINDOW.as_secs_f32();
            let sample = self.history.at_age(Instant::now(), age)?;
            let ago = Instant::now().duration_since(sample.at).as_secs();
            Some((x, sample.cpu_temp, sample.gpu_temp, ago))
        });
        let on_hover = MsiApp::handler(cx, |this, hover: Option<f32>, _, cx| {
            if this.chart_hover != hover {
                this.chart_hover = hover;
                cx.notify();
            }
        });

        let chart = div()
            .relative()
            .h(px(CHART_H))
            .w_full()
            .child(TimelineChart::new(&self.history, self.chart_hover).into_canvas(on_hover))
            .children([50.0f32, 70.0, 90.0].map(|g| {
                div()
                    .absolute()
                    .left_0()
                    .top(relative(1.0 - unit(g, TEMP_DOMAIN)))
                    .mt(px(-13.))
                    .child(mono(format!("{g:.0}°")).text_size(px(10.)).text_color(t.text_faint))
            }))
            .children(tooltip.map(|(x, cpu, gpu, ago)| {
                div()
                    .absolute()
                    .top(px(2.))
                    .when(x < 0.55, |d| d.left(relative(x)).ml(px(8.)))
                    .when(x >= 0.55, |d| d.right(relative(1.0 - x)).mr(px(8.)))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(8.))
                    .bg(t.raised)
                    .text_size(px(11.))
                    .child(
                        mono(format!("−{ago} с · CPU {} · GPU {}", fmt_temp(cpu), fmt_temp(gpu)))
                            .text_color(t.text),
                    )
            }));

        card(
            Icon::Chart,
            "Телеметрия",
            Some(mono("5 мин · 1,5 с").text_size(px(11.)).text_color(t.text_faint).into_any_element()),
        )
        .child(chart)
        .child(
            inset()
                .child(fan_row("CPU", false, &s.cpu, &self.history.recent_rpm(20, false), t.accent))
                .child(div().h(px(1.)).mx(px(10.)).bg(t.hairline))
                .child(fan_row("GPU", true, &s.gpu, &self.history.recent_rpm(20, true), t.secondary)),
        )
    }
}
