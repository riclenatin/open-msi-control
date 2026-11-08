//! Root view: owns all UI state and composes the views.
//!
//! Behaviour is split by concern into child modules, each adding an `impl MsiApp` block:
//! [`hardware`] (polling and EC writes), [`profiles`] (remember and restore), [`fan_curve`],
//! [`battery`] and [`shell`] (tray, window, app settings).

mod battery;
mod export;
mod fan_curve;
mod hardware;
mod profiles;
mod shell;
mod updates;

use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    AnyWindowHandle, App, Context, FocusHandle, MouseButton, RenderImage, SharedString, Task, Window,
    WindowHandle, actions, div, prelude::*,
};
use msi_acpi::{CURVE_POINTS, Fan, HwWorker, Info, Snapshot};

pub use battery::BATTERY_MIN;

use crate::i18n::{Lang, Strings};
use crate::motion::Transitions;
use crate::settings::Settings;
use crate::telemetry::History;
use crate::theme::theme;
use crate::tray::Tray;
use crate::ui::Handler;
use crate::views::Flyout;
use crate::{autostart, dither, platform};

actions!(open_msi_control, [FocusNext, FocusPrev]);

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
    pub(crate) settings: Settings,
    /// Last known power source; drives the active profile.
    pub(crate) on_ac: bool,
    /// Precision Touchpad state; `None` without one.
    pub(crate) touchpad: Option<bool>,
    /// Fan shown in the curve editor.
    pub(crate) curve_fan: Fan,
    /// Unapplied curve edits; `None` mirrors the EC.
    pub(crate) draft: Option<Curves>,
    pub(crate) curve_selected: usize,
    /// Battery limit being dragged or typed but not written yet (100 = no limit).
    pub(crate) battery_preview: Option<u8>,
    pub(crate) drag: Option<Drag>,
    pub(crate) error: Option<SharedString>,
    /// Non-error message (e.g. "CSV saved"), shown until the next action.
    pub(crate) notice: Option<SharedString>,
    pub(crate) chart_hover: Option<f32>,
    /// Index into [`crate::telemetry::RANGES`].
    pub(crate) chart_range: usize,
    pub(crate) transitions: Transitions,
    /// Focus rings are shown only while navigating with the keyboard.
    pub(crate) keyboard_nav: bool,
    /// The next frame snaps all transitions (input came from the keyboard).
    pub(crate) snap_frame: bool,
    pub(crate) dither: Arc<RenderImage>,
    /// Focus root for keyboard navigation. Tracked on the body, not the window root: gpui's
    /// focus-on-click calls `prevent_default`, which would swallow title-bar drags.
    pub(crate) focus: FocusHandle,
    /// Resolved UI language (setting, or the Windows display language).
    pub(crate) lang: Lang,
    /// Start-with-Windows task state; `None` while it is being queried.
    pub(crate) autostart: Option<bool>,
    /// Tray icon; `None` if it could not be created (then closing the window quits).
    tray: Option<Tray>,
    /// The main window, so the flyout can bring it back.
    main_window: AnyWindowHandle,
    flyout: Option<WindowHandle<Flyout>>,
    /// When the flyout last closed (clicking the tray icon closes it first; don't reopen).
    flyout_closed_at: Option<Instant>,
    /// Bumped on every write; polls requested before a write are discarded.
    write_gen: u64,
    /// Profile enforcement window after boot, wake or power change: `(until, forced)`, where
    /// `forced` means "apply even if restore is off" (power-source switch).
    enforce_until: Option<(Instant, bool)>,
    last_poll: Option<Instant>,
    last_enforce: Option<Instant>,
    _poll: Option<Task<()>>,
    battery_write: Option<Task<()>>,
    /// A newer GitHub release, once found.
    pub(crate) update: Option<crate::update::Release>,
}

impl MsiApp {
    /// `start_hidden`: launched by autostart; stay in the tray until the icon is clicked.
    pub fn new(window: &mut Window, cx: &mut Context<Self>, start_hidden: bool) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let settings = Settings::load();
        let lang = settings.language.unwrap_or_else(|| Lang::from_windows_ui(platform::ui_language()));
        let mut this = Self {
            conn: Connection::Connecting,
            info: None,
            model: platform::model_name(),
            snap: None,
            history: History::default(),
            settings,
            on_ac: platform::on_ac_power(),
            touchpad: platform::touchpad_enabled(),
            curve_fan: Fan::Cpu,
            draft: None,
            curve_selected: 0,
            battery_preview: None,
            drag: None,
            error: None,
            notice: None,
            chart_hover: None,
            chart_range: 0,
            transitions: Transitions::new(platform::reduced_motion()),
            keyboard_nav: false,
            snap_frame: false,
            dither: dither::render(),
            focus,
            lang,
            autostart: None,
            tray: None,
            main_window: window.window_handle(),
            flyout: None,
            flyout_closed_at: None,
            write_gen: 0,
            enforce_until: None,
            last_poll: None,
            last_enforce: None,
            _poll: None,
            battery_write: None,
            update: None,
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
        this.start_update_checks(cx);
        this
    }

    pub(crate) fn tr(&self) -> &'static Strings {
        self.lang.strings()
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
