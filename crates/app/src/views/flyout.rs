//! Tray flyout: a small always-on-top window with the most used controls.
//!
//! It owns no hardware state: it reads [`MsiApp`] (observing it for changes) and calls its
//! intents. It closes itself when it loses focus.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, Context, Entity, FontWeight, SharedString, WeakEntity, Window, div,
    prelude::*, px,
};
use msi_acpi::Scenario;

use super::header::gauge;
use crate::app::MsiApp;
use crate::assets::Icon;
use crate::motion::{self, EASE_OUT, Spec, Transitions};
use crate::theme::{FONT_MONO, radius, theme};
use crate::ui::{Button, Handler, SwitchRow, Variant, Width, inset, segmented};

pub const WIDTH: f32 = 340.;
pub const HEIGHT: f32 = 318.;
const PRESETS: [u8; 3] = [100, 80, 60];

pub struct Flyout {
    app: WeakEntity<MsiApp>,
    transitions: Transitions,
}

impl Flyout {
    pub fn new(app: &Entity<MsiApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(app, |_, _, cx| cx.notify()).detach();
        let weak = app.downgrade();
        cx.observe_window_activation(window, move |_, window, cx| {
            if !window.is_window_active() {
                weak.update(cx, |app, _| app.flyout_closed()).ok();
                window.remove_window();
            }
        })
        .detach();
        Self { app: app.downgrade(), transitions: Transitions::new(crate::platform::reduced_motion()) }
    }

    /// Callback that runs an app intent.
    fn call<E: 'static>(&self, f: impl Fn(&mut MsiApp, E, &mut Context<MsiApp>) + 'static) -> Handler<E> {
        let app = self.app.clone();
        Rc::new(move |e, _window: &mut Window, cx: &mut App| {
            app.update(cx, |a, cx| f(a, e, cx)).ok();
        })
    }
}

impl Render for Flyout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        let root = div().size_full().p(px(12.)).flex().flex_col().gap(px(10.)).bg(t.card).text_color(t.text);
        let Some(app) = self.app.upgrade() else { return root };
        let a = app.read(cx);
        let tr = a.tr();
        let Some(s) = a.snap.clone() else { return root };
        let battery = s.battery_limit.unwrap_or(100);
        self.transitions.begin_frame(false);

        let mut eased = |key, v: Option<u8>| {
            v.map(|v| self.transitions.animate(("gauge", key), f32::from(v), Spec::value(motion::VALUE)))
        };
        let (cpu, gpu) = (eased(0, s.cpu.temp_c), eased(1, s.gpu.temp_c));

        let current = s.scenario.and_then(|c| Scenario::ALL.iter().position(|&x| x == c));
        let scenario_at = current.map_or(0.0, |i| {
            self.transitions.animate(("scenario", 0), i as f32, Spec::movement(motion::MOVE))
        });
        let fan_at = self.transitions.animate(
            ("fan-mode", 0),
            f32::from(u8::from(s.advanced_fan)),
            Spec::movement(motion::MOVE),
        );
        let preset = PRESETS.iter().position(|&p| p == battery);
        let preset_at = preset.map_or(0.0, |i| {
            self.transitions.animate(("battery", 0), i as f32, Spec::movement(motion::MOVE))
        });
        let boost_at = self.transitions.animate(
            ("boost", 0),
            f32::from(u8::from(s.cooler_boost)),
            Spec::movement(motion::TOGGLE),
        );

        let scenarios = segmented(
            "fly-scenario",
            &tr.scenarios.map(SharedString::from),
            current,
            scenario_at,
            Width::Fill,
            false,
            window,
            self.call(|a, (i, kb): (usize, bool), cx| a.set_scenario(Scenario::ALL[i], kb, cx)),
        );
        let fan_mode = segmented(
            "fly-fan",
            &[SharedString::from(tr.fan_auto), SharedString::from(tr.fan_custom)],
            Some(usize::from(s.advanced_fan)),
            fan_at,
            Width::Content,
            false,
            window,
            self.call(
                |a, (i, _): (usize, bool), cx| if i == 0 { a.auto_fan(cx) } else { a.apply_curves(cx) },
            ),
        );
        let presets = segmented(
            "fly-battery",
            &PRESETS.map(|p| SharedString::from(format!("{p} %"))),
            preset,
            preset_at,
            Width::Content,
            false,
            window,
            self.call(|a, (i, _): (usize, bool), cx| a.set_battery(PRESETS[i], false, cx)),
        );
        let boost = SwitchRow {
            id: "fly-boost".into(),
            icon: Icon::Wind,
            label: "Cooler Boost",
            description: None,
            explain: None,
            on: s.cooler_boost,
            thumb_at: boost_at,
            ring: false,
            on_toggle: self.call(|a, (on, kb): (bool, bool), cx| a.set_cooler_boost(on, kb, cx)),
        }
        .render();

        if self.transitions.is_animating() {
            window.request_animation_frame();
        }

        let label = |text: &'static str| div().text_size(px(12.)).text_color(t.text_muted).child(text);
        let open = self.app.clone();
        let quit = self.app.clone();
        root.child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .size_full()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).child(
                                        a.model.clone().unwrap_or_else(|| tr.model_fallback.to_owned()),
                                    ),
                                )
                                .child(
                                    div()
                                        .font_family(FONT_MONO)
                                        .text_size(px(11.))
                                        .text_color(t.text_faint)
                                        .child(format!("{} rpm · {} rpm", s.cpu.rpm, s.gpu.rpm)),
                                ),
                        )
                        .child(div().flex().gap(px(2.)).child(gauge("CPU", cpu, s.cpu.temp_c)).child(gauge(
                            "GPU",
                            gpu,
                            s.gpu.temp_c,
                        ))),
                )
                .child(scenarios)
                .child(div().flex().items_center().justify_between().child(label(tr.fans)).child(fan_mode))
                .child(inset().p(px(2.)).child(boost))
                .child(div().flex().items_center().justify_between().child(label(tr.battery)).child(presets))
                .child(div().flex_1())
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(div().flex_1().child(
                            Button::new("fly-open", tr.open_app, Variant::Primary).on_click(
                                move |_, _, cx| {
                                    open.update(cx, |a, cx| a.show_main_window(cx)).ok();
                                },
                            ),
                        ))
                        .child(Button::new("fly-quit", tr.tray_exit, Variant::Secondary).on_click(
                            move |_, _, cx| {
                                quit.update(cx, |a, cx| a.quit(cx)).ok();
                            },
                        )),
                )
                // Popover entrance: a quick fade (Emil: 125–200 ms, ease-out).
                .with_animation(
                    "fly-in",
                    Animation::new(Duration::from_millis(150)).with_easing(|x| EASE_OUT.ease(x)),
                    |el, t| el.opacity(t),
                ),
        )
        .rounded(radius::card())
    }
}
