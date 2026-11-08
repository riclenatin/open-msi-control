//! Root view: owns all UI state, talks to the hardware worker, and composes the views.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Context, FocusHandle, MouseButton, RenderImage, SharedString, Task, Window, actions, div, prelude::*,
};
use msi_acpi::{CURVE_POINTS, Fan, HwWorker, Info, MsiLaptop, Scenario, Snapshot};

use crate::i18n::{Lang, Strings};
use crate::motion::Transitions;
use crate::telemetry::{History, POLL};
use crate::theme::theme;
use crate::tray::{MenuItem, Tray, TrayEvent};
use crate::ui::{DragEvent, Handler};
use crate::{autostart, dither, platform};

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    /// Dragging a fan-curve point: which point, the curves when the drag started and the
    /// pointer's starting height (0..=1, y down).
    Curve {
        point: usize,
        base: Curves,
        start_y: f32,
    },
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
    /// Focus root for keyboard navigation. Tracked on the body, not the window root: gpui's
    /// focus-on-click calls `prevent_default`, which would swallow title-bar drags.
    pub(crate) focus: FocusHandle,
    pub(crate) lang: Lang,
    /// Tray icon; `None` if it could not be created (then closing the window quits).
    tray: Option<Tray>,
    pub(crate) close_to_tray: bool,
    /// Start-with-Windows task state; `None` while it is being queried.
    pub(crate) autostart: Option<bool>,
    /// Bumped on every write; polls requested before a write are discarded.
    write_gen: u64,
    _poll: Option<Task<()>>,
    battery_write: Option<Task<()>>,
}

impl MsiApp {
    /// `start_hidden`: launched by autostart; stay in the tray until the icon is clicked.
    pub fn new(window: &mut Window, cx: &mut Context<Self>, start_hidden: bool) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let lang = platform::load_setting("language")
            .and_then(|code| Lang::from_code(&code))
            .unwrap_or_else(|| Lang::from_windows_ui(platform::ui_language()));
        let mut this = Self {
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
            lang,
            tray: None,
            close_to_tray: platform::load_setting("close_to_tray").is_none_or(|v| v != "0"),
            autostart: None,
            write_gen: 0,
            _poll: None,
            battery_write: None,
        };
        this.init_tray(window, cx, start_hidden);
        cx.spawn(async move |this, cx| {
            let res = cx.background_executor().spawn(async move { HwWorker::spawn() }).await;
            this.update(cx, |this, cx| this.on_connected(res, cx)).ok();
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let enabled = cx.background_executor().spawn(async { autostart::is_enabled() }).await;
            this.update(cx, |this, cx| {
                this.autostart = Some(enabled);
                cx.notify();
            })
            .ok();
        })
        .detach();
        this
    }

    fn init_tray(&mut self, window: &mut Window, cx: &mut Context<Self>, start_hidden: bool) {
        match Tray::create(&(self.tr().tray_tooltip)(None, None)) {
            Ok((tray, events)) => {
                self.tray = Some(tray);
                cx.spawn_in(window, async move |this, cx| {
                    while let Ok(event) = events.recv_async().await {
                        if this.update_in(cx, |this, window, cx| this.on_tray(event, window, cx)).is_err() {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(e) => {
                eprintln!("tray icon unavailable: {e}");
                // Without a tray there would be no way back to a hidden window.
                if start_hidden {
                    platform::show_window(window);
                }
            }
        }
        // The close button hides to the tray (if enabled); otherwise remove the icon and quit.
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let hide = weak
                .update(cx, |this, _| {
                    let hide = this.tray.is_some() && this.close_to_tray;
                    if !hide {
                        this.tray = None;
                    }
                    hide
                })
                .unwrap_or(false);
            if hide {
                platform::hide_window(window);
            }
            !hide
        });
    }

    fn on_tray(&mut self, event: TrayEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            TrayEvent::Toggle if platform::is_window_visible(window) => platform::hide_window(window),
            TrayEvent::Toggle | TrayEvent::ShowRequested => platform::show_window(window),
            TrayEvent::Menu => self.tray_menu(window, cx),
        }
    }

    /// Quick actions: open, scenarios, Cooler Boost, quit.
    fn tray_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tray) = &self.tray else { return };
        let tr = self.tr();
        let snap = self.snap.as_ref();
        let action = |label: &str, checked: bool| MenuItem::Action { label: label.to_owned(), checked };
        let mut items = vec![action(tr.tray_open, false), MenuItem::Separator];
        for (i, sc) in Scenario::ALL.into_iter().enumerate() {
            items.push(action(tr.scenarios[i], snap.and_then(|s| s.scenario) == Some(sc)));
        }
        items.push(MenuItem::Separator);
        items.push(action("Cooler Boost", snap.is_some_and(|s| s.cooler_boost)));
        items.push(MenuItem::Separator);
        items.push(action(tr.tray_exit, false));

        const SCENARIOS: usize = 2;
        let boost = SCENARIOS + Scenario::ALL.len() + 1;
        let exit = boost + 2;
        match tray.show_menu(&items) {
            Some(0) => platform::show_window(window),
            Some(i) if (SCENARIOS..SCENARIOS + Scenario::ALL.len()).contains(&i) => {
                self.set_scenario(Scenario::ALL[i - SCENARIOS], false, cx);
            }
            Some(i) if i == boost => {
                let on = !self.snap.as_ref().is_some_and(|s| s.cooler_boost);
                self.set_cooler_boost(on, false, cx);
            }
            Some(i) if i == exit => {
                self.tray = None;
                cx.quit();
            }
            _ => {}
        }
    }

    pub(crate) fn set_close_to_tray(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.close_to_tray = on;
        platform::save_setting("close_to_tray", if on { "1" } else { "0" });
        cx.notify();
    }

    pub(crate) fn set_autostart(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.autostart = Some(on);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let res = cx
                .background_executor()
                .spawn(async move { autostart::set_enabled(on).map(|()| autostart::is_enabled()) })
                .await;
            this.update(cx, |this, cx| {
                match res {
                    Ok(actual) => this.autostart = Some(actual),
                    Err(e) => {
                        this.error = Some(format!("{}: {e}", this.tr().err_autostart).into());
                        this.autostart = Some(!on);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn tr(&self) -> &'static Strings {
        self.lang.strings()
    }

    pub(crate) fn set_lang(&mut self, lang: Lang, cx: &mut Context<Self>) {
        self.lang = lang;
        platform::save_setting("language", lang.code());
        cx.notify();
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
                            if let (Some(tray), Some(s)) = (&this.tray, &this.snap) {
                                tray.set_tooltip(&(this.tr().tray_tooltip)(s.cpu.temp_c, s.gpu.temp_c));
                            }
                        }
                        Err(e) => this.error = Some(format!("{}: {e}", this.tr().read_failed).into()),
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
        what: fn(&Strings) -> &'static str,
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
                    Err(e) => this.error = Some(format!("{}: {e}", what(this.tr())).into()),
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
        self.act(|t| t.err_scenario, |snap| snap.scenario = Some(s), move |l| l.set_scenario(s), cx);
    }

    pub(crate) fn set_cooler_boost(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_cooler_boost, |s| s.cooler_boost = on, move |l| l.set_cooler_boost(on), cx);
    }

    pub(crate) fn set_kbd_backlight(&mut self, level: u8, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_backlight, |s| s.kbd_backlight = level, move |l| l.set_kbd_backlight(level), cx);
    }

    pub(crate) fn set_webcam(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_webcam, |s| s.webcam = Some(on), move |l| l.set_webcam(on), cx);
    }

    pub(crate) fn set_win_fn(&mut self, swapped: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_win_fn, |s| s.win_fn_swapped = swapped, move |l| l.set_win_fn_swap(swapped), cx);
    }

    pub(crate) fn set_usb_share(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_usb, |s| s.usb_share = Some(on), move |l| l.set_usb_share(on), cx);
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

    /// Sets point `i` on top of `base` and keeps the curve monotonic. Re-deriving from `base`
    /// (the curve when the drag started) lets neighbours spring back if the point moves back.
    fn set_point_on(&mut self, base: Curves, i: usize, duty: u8, cx: &mut Context<Self>) {
        let mut c = base;
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
        self.set_point_on(c, self.curve_selected, (cur + delta).clamp(0, 100) as u8, cx);
    }

    pub(crate) fn apply_curves(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.curves() else { return };
        self.draft = None;
        self.act(
            |t| t.err_curve,
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
        self.act(|t| t.err_auto_fan, |s| s.advanced_fan = false, |l| l.set_auto_fan(), cx);
    }

    pub(crate) fn on_curve_drag(
        &mut self,
        e: DragEvent,
        chart: &crate::charts::CurveChart,
        cx: &mut Context<Self>,
    ) {
        match e {
            // Pressing only grabs the band; the value changes by how far the pointer moves.
            DragEvent::Press(p) => {
                if let (Some(point), Some(base)) = (chart.band_at(p.x), self.curves()) {
                    self.drag = Some(Drag::Curve { point, base, start_y: p.y });
                    self.curve_selected = point;
                    cx.notify();
                }
            }
            DragEvent::Move(p) => {
                if let Some(Drag::Curve { point, base, start_y }) = self.drag {
                    let start = f32::from(base[self.fan_index()][point]);
                    let duty = (start + (start_y - p.y) * 100.0).round().clamp(0.0, 100.0) as u8;
                    self.set_point_on(base, point, duty, cx);
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
        self.act(|t| t.err_battery, |s| s.battery_limit = limit, move |l| l.set_battery_limit(limit), cx);
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
            .child(div().flex_1().min_h_0().flex().flex_col().track_focus(&self.focus).child(body))
    }
}
