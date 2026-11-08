use gpui::{Context, KeyDownEvent, SharedString, Window, div, prelude::*, px, relative, transparent_black};
use msi_acpi::{CURVE_POINTS, Fan};

use crate::app::{Drag, MsiApp};
use crate::assets::Icon;
use crate::charts::{CurveChart, TEMP_MAX, TEMP_MIN, unit};
use crate::motion::{self, Spec};
use crate::theme::theme;
use crate::ui::{Button, DragEvent, Variant, Width, card, inset, mono, segmented, switch_row};

const CHART_H: f32 = 132.;

impl MsiApp {
    pub(crate) fn render_fans(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let Some(s) = self.snap.clone() else { return div() };
        let Some(curves) = self.curves() else { return div() };
        let fan = self.fan_index();
        let status = if fan == 0 { &s.cpu } else { &s.gpu };
        let dirty = self.draft.is_some();
        let ring = self.keyboard_nav;
        let dragging = matches!(self.drag, Some(Drag::Curve(_)));

        let chart = CurveChart {
            temps: status.curve.temps,
            speeds: curves[fan],
            top: status.curve.top,
            current_temp: status.temp_c,
            selected: (self.keyboard_nav || dragging).then_some(self.curve_selected),
            dragging,
        };

        let tab_at = self.transitions.animate(("fan-tab", 0), fan as f32, Spec::movement(motion::MOVE));
        let tabs = segmented(
            "fan-tab",
            &[SharedString::from("CPU"), SharedString::from("GPU")],
            Some(fan),
            tab_at,
            Width::Content,
            ring,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.snap_frame |= kb;
                this.curve_fan = if i == 0 { Fan::Cpu } else { Fan::Gpu };
                cx.notify();
            }),
        );

        // Duty above each handle, band start temperatures underneath.
        let labels =
            (0..=CURVE_POINTS).map(|i| {
                let (x, y) = chart.handle(i);
                div()
                    .absolute()
                    .left(relative(x))
                    .top(relative(1.0 - y))
                    .ml(px(-16.))
                    .mt(px(-22.))
                    .w(px(32.))
                    .flex()
                    .justify_center()
                    .child(
                        mono(format!("{}", chart.speed(i)))
                            .text_size(px(10.))
                            .text_color(if i == CURVE_POINTS { t.text_faint } else { t.text_muted }),
                    )
            });
        let temps = (1..=CURVE_POINTS).map(|i| {
            let temp = if i == CURVE_POINTS { chart.top.0 } else { chart.temps[i] };
            div()
                .absolute()
                .left(relative(unit(f32::from(temp), (TEMP_MIN, TEMP_MAX))))
                .ml(px(-14.))
                .w(px(28.))
                .flex()
                .justify_center()
                .child(mono(format!("{temp}°")).text_size(px(10.)).text_color(t.text_faint))
        });

        let drag_chart = chart.clone();
        let on_drag =
            MsiApp::handler(cx, move |this, e: DragEvent, _, cx| this.on_curve_drag(e, &drag_chart, cx));
        let editor = div()
            .id("curve-editor")
            .tab_index(0)
            .flex()
            .flex_col()
            .gap(px(4.))
            .px(px(2.))
            .rounded(px(10.))
            .border_2()
            .border_color(transparent_black())
            .when(ring, |d| d.focus(|st| st.border_color(t.focus)))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let step = if e.keystroke.modifiers.shift { 5 } else { 1 };
                match e.keystroke.key.as_str() {
                    "left" => this.curve_selected = this.curve_selected.saturating_sub(1),
                    "right" => this.curve_selected = (this.curve_selected + 1).min(CURVE_POINTS - 1),
                    "up" => this.nudge_point(step, cx),
                    "down" => this.nudge_point(-step, cx),
                    "enter" if this.draft.is_some() => this.apply_curves(cx),
                    "escape" => this.draft = None,
                    _ => return,
                }
                this.keyboard_nav = true;
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .relative()
                    .h(px(CHART_H))
                    .mt(px(16.))
                    .child(chart.clone().into_canvas(on_drag))
                    .children(labels),
            )
            .child(div().relative().h(px(12.)).children(temps));

        let mode_at = self.transitions.animate(
            ("fan-mode", 0),
            f32::from(u8::from(s.advanced_fan)),
            Spec::movement(motion::MOVE),
        );
        let mode = segmented(
            "fan-mode",
            &[SharedString::from("Авто"), SharedString::from("Своя кривая")],
            Some(usize::from(s.advanced_fan)),
            mode_at,
            Width::Content,
            ring,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.snap_frame |= kb;
                if i == 0 { this.auto_fan(cx) } else { this.apply_curves(cx) }
            }),
        );

        let boost_at = self.transitions.animate(
            ("boost", 0),
            f32::from(u8::from(s.cooler_boost)),
            Spec::movement(motion::TOGGLE),
        );
        let boost = switch_row(
            "cooler-boost",
            Icon::Wind,
            "Cooler Boost",
            Some("Максимальные обороты поверх кривой".into()),
            s.cooler_boost,
            boost_at,
            ring,
            MsiApp::handler(cx, |this, (on, kb): (bool, bool), _, cx| this.set_cooler_boost(on, kb, cx)),
        );

        let readout = format!(
            "цель {}% · {}",
            status.duty,
            status.temp_c.map_or("датчик спит".to_owned(), |v| format!("{v}°"))
        );
        card(Icon::Fan, "Вентиляторы", Some(tabs.into_any_element()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(mono(readout).text_size(px(12.)).text_color(t.accent))
                    .child(div().text_size(px(11.)).text_color(t.text_faint).child("тяните точки")),
            )
            .child(editor)
            .child(div().flex().items_center().gap(px(8.)).child(mode).child(div().flex_1()).when(
                dirty,
                |d| {
                    d.child(
                        Button::new("curve-reset", "Сброс", Variant::Secondary)
                            .icon(Icon::Reset)
                            .ring(ring)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.draft = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("curve-apply", "Применить", Variant::Primary)
                            .ring(ring)
                            .on_click(cx.listener(|this, _, _, cx| this.apply_curves(cx))),
                    )
                },
            ))
            .child(inset().child(boost))
    }
}
