//! Root view: owns all UI state, talks to the hardware worker, and composes the views.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Context, FocusHandle, MouseButton, RenderImage, SharedString, Task, Window, actions, div, prelude::*,
};
use msi_acpi::{CURVE_POINTS, Fan, HwWorker, Info, MsiLaptop, Scenario, Snapshot};

use crate::motion::Transitions;
use crate::telemetry::{History, POLL};
use crate::theme::theme;
use crate::ui::{DragEvent, Handler};
use crate::{dither, platform};

actions!(open_msi_control, [FocusNext, FocusPrev]);

/// Delay before a keyboard-adjusted battery limit is written.
const BATTERY_DEBOUNCE: Duration = Duration::from_millis(450);
pub const BATTERY_MIN: u8 = 40;

pub type Curves = [[u8; CURVE_POINTS]; 2];

#[derive(Debug)]
pub enum Connection {
    Connecting,
    Ready(HwWorker),
    Failed(SharedString),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drag {
    Curve(usize),
    Battery,
}

pub struct MsiApp {
    pub(crate) conn: Connection,
    pub(crate) info: Option<Info>,
    pub(crate) model: Option<String>,
    pub(crate) snap: Option<Snapshot>,
    pub(crate) history: History,
    /// Fan shown in the curve editor.
    pub(crate) curve_fan: Fan,
    /// Unapplied curve edits; `None` mirrors the EC.
    pub(crate) draft: Option<Curves>,
    pub(crate) curve_selected: usize,
    /// Battery limit being dragged or typed but not written yet (100 = no limit).
    pub(crate) battery_preview: Option<u8>,
    pub(crate) drag: Option<Drag>,
    pub(crate) error: Option<SharedString>,
    pub(crate) chart_hover: Option<f32>,
    pub(crate) transitions: Transitions,
    /// Focus rings are shown only while navigating with the keyboard.
    pub(crate) keyboard_nav: bool,
    /// The next frame snaps all transitions (input came from the keyboard).
    pub(crate) snap_frame: bool,
    pub(crate) dither: Arc<RenderImage>,
    pub(crate) focus: FocusHandle,
    /// Bumped on every write; polls requested before a write are discarded.
    write_gen: u64,
    _poll: Option<Task<()>>,
    battery_write: Option<Task<()>>,
}

impl MsiApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let this = Self {
            conn: Connection::Connecting,
            info: None,
            model: platform::model_name(),
            snap: None,
            history: History::default(),
            curve_fan: Fan::Cpu,
            draft: None,
            curve_selected: 0,
            battery_preview: None,
            drag: None,
            error: None,
            chart_hover: None,
            transitions: Transitions::new(platform::reduced_motion()),
            keyboard_nav: false,
            snap_frame: false,
            dither: dither::render(),
            focus,
            write_gen: 0,
            _poll: None,
            battery_write: None,
        };
        cx.spawn(async move |this, cx| {
            let res = cx.background_executor().spawn(async move { HwWorker::spawn() }).await;
            this.update(cx, |this, cx| this.on_connected(res, cx)).ok();
        })
        .detach();
        this
    }

    fn on_connected(&mut self, res: msi_acpi::Result<HwWorker>, cx: &mut Context<Self>) {
        match res {
            Ok(hw) => {
                let poll = self.start_polling(hw.clone(), cx);
                self._poll = Some(poll);
                self.conn = Connection::Ready(hw);
            }
            Err(e) => self.conn = Connection::Failed(e.to_string().into()),
        }
        cx.notify();
    }

    fn start_polling(&self, hw: HwWorker, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(generation) = this.update(cx, |this, _| this.write_gen) else { break };
                let worker = hw.clone();
                let res = cx
                    .background_executor()
                    .spawn(async move { worker.run(|l| Ok((l.info().clone(), l.snapshot()?))) })
                    .await;
                let alive = this.update(cx, |this, cx| {
                    match res {
                        Ok((info, snap)) => {
                            this.info = Some(info);
                            this.history.push(&snap);
                            if this.write_gen == generation {
                                this.snap = Some(snap);
                            }
                        }
                        Err(e) => this.error = Some(format!("Не удалось прочитать датчики: {e}").into()),
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
                cx.background_executor().timer(POLL).await;
            }
        })
    }

    /// Applies `predict` to the local snapshot immediately, then performs `write` on the
    /// hardware thread and replaces the snapshot with what the EC reports.
    pub(crate) fn act(
        &mut self,
        what: &'static str,
        predict: impl FnOnce(&mut Snapshot),
        write: impl FnOnce(&MsiLaptop) -> msi_acpi::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Connection::Ready(hw) = &self.conn else { return };
        let hw = hw.clone();
        if let Some(s) = &mut self.snap {
            predict(s);
        }
        self.write_gen += 1;
        let generation = self.write_gen;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move {
                    hw.run(move |l| {
                        write(l)?;
                        l.snapshot()
                    })
                })
                .await;
            this.update(cx, |this, cx| {
                match res {
                    Ok(s) if this.write_gen == generation => this.snap = Some(s),
                    Ok(_) => {}
                    Err(e) => this.error = Some(format!("{what}: {e}").into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Wraps an app method as a `'static` callback for components.
    pub(crate) fn handler<E: 'static>(
        cx: &Context<Self>,
        f: impl Fn(&mut Self, E, &mut Window, &mut Context<Self>) + 'static,
    ) -> Handler<E> {
        let view = cx.entity().downgrade();
        Rc::new(move |e, window, cx: &mut App| {
            view.update(cx, |this, cx| f(this, e, window, cx)).ok();
        })
    }

    // ---- intents -------------------------------------------------------------------------

    pub(crate) fn set_scenario(&mut self, s: Scenario, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act("Сценарий", |snap| snap.scenario = Some(s), move |l| l.set_scenario(s), cx);
    }

    pub(crate) fn set_cooler_boost(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act("Cooler Boost", |s| s.cooler_boost = on, move |l| l.set_cooler_boost(on), cx);
    }

    pub(crate) fn set_kbd_backlight(&mut self, level: u8, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act("Подсветка", |s| s.kbd_backlight = level, move |l| l.set_kbd_backlight(level), cx);
    }

    pub(crate) fn set_webcam(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act("Веб-камера", |s| s.webcam = Some(on), move |l| l.set_webcam(on), cx);
    }

    pub(crate) fn set_win_fn(&mut self, swapped: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act("Win/Fn", |s| s.win_fn_swapped = swapped, move |l| l.set_win_fn_swap(swapped), cx);
    }

    pub(crate) fn set_usb_share(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act("Зарядка по USB", |s| s.usb_share = Some(on), move |l| l.set_usb_share(on), cx);
    }

    // ---- fan curve -----------------------------------------------------------------------

    pub(crate) fn curves(&self) -> Option<Curves> {
        self.draft.or_else(|| self.snap.as_ref().map(|s| [s.cpu.curve.speeds, s.gpu.curve.speeds]))
    }

    pub(crate) fn fan_index(&self) -> usize {
        match self.curve_fan {
            Fan::Cpu => 0,
            Fan::Gpu => 1,
        }
    }

    pub(crate) fn set_point(&mut self, i: usize, duty: u8, cx: &mut Context<Self>) {
        let Some(mut c) = self.curves() else { return };
        let fan = self.fan_index();
        // Never above the firmware's top point, or the fan would slow down when hottest.
        let ceiling = self.snap.as_ref().map_or(100, |s| match self.curve_fan {
            Fan::Cpu => s.cpu.curve.top.1,
            Fan::Gpu => s.gpu.curve.top.1,
        });
        let duty = duty.min(ceiling);
        let row = &mut c[fan];
        row[i] = duty;
        // Keep the curve non-decreasing by dragging neighbours along.
        for s in &mut row[i + 1..] {
            *s = (*s).max(duty);
        }
        for s in &mut row[..i] {
            *s = (*s).min(duty);
        }
        self.curve_selected = i;
        self.draft = Some(c);
        cx.notify();
    }

    pub(crate) fn nudge_point(&mut self, delta: i16, cx: &mut Context<Self>) {
        let Some(c) = self.curves() else { return };
        let cur = i16::from(c[self.fan_index()][self.curve_selected]);
        self.set_point(self.curve_selected, (cur + delta).clamp(0, 100) as u8, cx);
    }

    pub(crate) fn apply_curves(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.curves() else { return };
        self.draft = None;
        self.act(
            "Кривая вентиляторов",
            |s| {
                s.cpu.curve.speeds = c[0];
                s.gpu.curve.speeds = c[1];
                s.advanced_fan = true;
            },
            move |l| l.set_fan_curves(c[0], c[1]),
            cx,
        );
    }

    pub(crate) fn auto_fan(&mut self, cx: &mut Context<Self>) {
        self.draft = None;
        self.act("Авто-режим вентиляторов", |s| s.advanced_fan = false, |l| l.set_auto_fan(), cx);
    }

    pub(crate) fn on_curve_drag(
        &mut self,
        e: DragEvent,
        chart: &crate::charts::CurveChart,
        cx: &mut Context<Self>,
    ) {
        match e {
            DragEvent::Press(p) => {
                if let Some(i) = chart.band_at(p.x) {
                    self.drag = Some(Drag::Curve(i));
                    self.set_point(i, crate::charts::CurveChart::duty_at(p.y), cx);
                }
            }
            DragEvent::Move(p) => {
                if let Some(Drag::Curve(i)) = self.drag {
                    self.set_point(i, crate::charts::CurveChart::duty_at(p.y), cx);
                }
            }
            DragEvent::Release => {
                self.drag = None;
                cx.notify();
            }
        }
    }

    // ---- battery -------------------------------------------------------------------------

    pub(crate) fn battery_value(&self) -> u8 {
        self.battery_preview
            .or_else(|| self.snap.as_ref().map(|s| s.battery_limit.unwrap_or(100)))
            .unwrap_or(100)
    }

    fn write_battery(&mut self, value: u8, cx: &mut Context<Self>) {
        self.battery_write = None;
        let limit = (value < 100).then_some(value);
        self.act("Лимит заряда", |s| s.battery_limit = limit, move |l| l.set_battery_limit(limit), cx);
        self.battery_preview = None;
    }

    pub(crate) fn set_battery(&mut self, value: u8, debounce: bool, cx: &mut Context<Self>) {
        let value = value.clamp(BATTERY_MIN, 100);
        if !debounce {
            self.write_battery(value, cx);
            return;
        }
        self.battery_preview = Some(value);
        self.battery_write = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BATTERY_DEBOUNCE).await;
            this.update(cx, |this, cx| this.write_battery(value, cx)).ok();
        }));
        cx.notify();
    }

    pub(crate) fn on_battery_drag(&mut self, e: DragEvent, cx: &mut Context<Self>) {
        let to_value = |x: f32| (f32::from(BATTERY_MIN) + x * f32::from(100 - BATTERY_MIN)).round() as u8;
        match e {
            DragEvent::Press(p) => {
                self.drag = Some(Drag::Battery);
                self.battery_write = None;
                self.battery_preview = Some(to_value(p.x));
            }
            DragEvent::Move(p) if self.drag == Some(Drag::Battery) => {
                self.battery_preview = Some(to_value(p.x));
            }
            DragEvent::Release if self.drag == Some(Drag::Battery) => {
                self.drag = None;
                let v = self.battery_value();
                self.write_battery(v, cx);
            }
            _ => return,
        }
        cx.notify();
    }
}

impl Render for MsiApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap_frame = std::mem::take(&mut self.snap_frame);
        self.transitions.begin_frame(snap_frame);

        let body = match &self.conn {
            Connection::Ready(_) if self.snap.is_some() => {
                self.render_dashboard(window, cx).into_any_element()
            }
            Connection::Failed(e) => self.render_failure(e.clone()).into_any_element(),
            _ => self.render_connecting().into_any_element(),
        };

        if self.transitions.is_animating() {
            window.request_animation_frame();
        }

        div()
            .id("root")
            .track_focus(&self.focus)
            .key_context("App")
            .on_action(cx.listener(|this, _: &FocusNext, window, cx| {
                this.keyboard_nav = true;
                window.focus_next();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusPrev, window, cx| {
                this.keyboard_nav = true;
                window.focus_prev();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.keyboard_nav {
                        this.keyboard_nav = false;
                        cx.notify();
                    }
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .bg(theme().background)
            .text_color(theme().text)
            .font_family(crate::theme::FONT_SANS)
            .child(self.render_header(window, cx))
            .child(body)
    }
}
