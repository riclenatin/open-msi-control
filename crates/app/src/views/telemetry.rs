use std::time::Instant;

use gpui::{
    Context, Div, Rgba, SharedString, Window, div, linear_color_stop, linear_gradient, prelude::*, px,
    relative,
};
use msi_acpi::FanStatus;

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::charts::{TEMP_DOMAIN, TEMP_HOT, TimelineChart, bars, segment_meter, unit};
use crate::motion::{self, Spec};
use crate::telemetry::{POLL, RANGES};
use crate::theme::theme;
use crate::ui::{Button, Variant, Width, card, inset, mono, segmented, tnum};

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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let tr = self.tr();
        let Some(s) = self.snap.clone() else { return div() };
        let label_fade = Rgba { a: 0.0, ..t.card };
        let range = RANGES[self.chart_range];

        let range_at = self.transitions.animate(
            ("chart-range", 0),
            self.chart_range as f32,
            Spec::movement(motion::MOVE),
        );
        let ranges = segmented(
            "chart-range",
            &tr.ranges.map(SharedString::from),
            Some(self.chart_range),
            range_at,
            Width::Content,
            self.keyboard_nav,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.snap_frame |= kb;
                this.chart_range = i;
                this.chart_hover = None;
                cx.notify();
            }),
        );

        let tooltip = self.chart_hover.and_then(|x| {
            let age = (1.0 - x) * range.as_secs_f32();
            // About two chart columns, never less than two polls.
            let tolerance = (range / 180).max(POLL * 2);
            let sample = self.history.at_age(Instant::now(), age, tolerance)?;
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
            .child(TimelineChart::new(&self.history, range, self.chart_hover).into_canvas(on_hover))
            // Axis labels sit on a fade so the lines passing behind them stay out of the way.
            .child(div().absolute().top_0().bottom_0().left_0().w(px(44.)).bg(linear_gradient(
                90.,
                linear_color_stop(t.card, 0.35),
                linear_color_stop(label_fade, 1.),
            )))
            .children([50.0f32, 70.0, TEMP_HOT].map(|g| {
                div().absolute().left_0().top(relative(1.0 - unit(g, TEMP_DOMAIN))).mt(px(-13.)).child(
                    mono(format!("{g:.0}°")).text_size(px(10.)).text_color(if g >= TEMP_HOT {
                        t.danger
                    } else {
                        t.text_faint
                    }),
                )
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
                        mono(format!(
                            "{} · CPU {} · GPU {}",
                            (tr.seconds_ago)(ago),
                            fmt_temp(cpu),
                            fmt_temp(gpu)
                        ))
                        .text_color(t.text),
                    )
            }));

        card(Icon::Chart, tr.telemetry, Some(tr.telemetry_info), Some(ranges.into_any_element()))
            .child(chart)
            .child(
                inset()
                    .child(fan_row("CPU", false, &s.cpu, &self.history.recent_rpm(20, false), t.accent))
                    .child(div().h(px(1.)).mx(px(10.)).bg(t.hairline))
                    .child(fan_row("GPU", true, &s.gpu, &self.history.recent_rpm(20, true), t.secondary)),
            )
            .child(
                div().flex().justify_end().child(
                    Button::new("export-csv", tr.export_csv, Variant::Secondary)
                        .icon(Icon::Download)
                        .ring(self.keyboard_nav)
                        .on_click(cx.listener(|this, _, _, cx| this.export_csv(cx))),
                ),
            )
    }
}
