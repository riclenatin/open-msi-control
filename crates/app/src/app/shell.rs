//! Desktop integration: tray icon and menu, window hiding, app-level settings.

use std::time::{Duration, Instant};

use gpui::{
    AppContext as _, Bounds, Context, Window, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, point, px, size,
};
use msi_acpi::Scenario;

use super::MsiApp;
use crate::i18n::Lang;
use crate::tray::{MenuItem, Tray, TrayEvent};
use crate::views::{Flyout, flyout};
use crate::{autostart, platform};

impl MsiApp {
    pub(super) fn init_tray(&mut self, window: &mut Window, cx: &mut Context<Self>, start_hidden: bool) {
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
                    let hide = this.tray.is_some() && this.settings.close_to_tray;
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
            TrayEvent::Toggle => self.toggle_flyout(cx),
            TrayEvent::ShowRequested => platform::show_window(window),
            TrayEvent::Menu => self.tray_menu(window, cx),
        }
    }

    /// Left click on the tray icon: open the flyout, or close it if open.
    fn toggle_flyout(&mut self, cx: &mut Context<Self>) {
        if let Some(h) = self.flyout.take() {
            h.update(cx, |_, window, _| window.remove_window()).ok();
            return;
        }
        // Clicking the icon deactivates (and so closes) the flyout before the click arrives here.
        if self.flyout_closed_at.is_some_and(|t| t.elapsed() < Duration::from_millis(300)) {
            return;
        }
        let (x, y) = platform::tray_popup_origin(flyout::WIDTH, flyout::HEIGHT);
        let app = cx.entity();
        // Deferred: the new window renders right away and reads `MsiApp`, which is leased here.
        cx.defer(move |cx| {
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(x), px(y)),
                        size: size(px(flyout::WIDTH), px(flyout::HEIGHT)),
                    })),
                    titlebar: None,
                    kind: WindowKind::PopUp,
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
                    window_background: WindowBackgroundAppearance::Opaque,
                    // Opened by the user's click on the icon, so taking focus is expected here.
                    focus: true,
                    show: true,
                    ..Default::default()
                },
                |window, cx| {
                    platform::style_popup(window);
                    cx.new(|cx| Flyout::new(&app, window, cx))
                },
            );
            match opened {
                Ok(h) => app.update(cx, |this, _| this.flyout = Some(h)),
                Err(e) => eprintln!("flyout: {e}"),
            }
        });
    }

    /// Called by the flyout when it closes itself.
    pub(crate) fn flyout_closed(&mut self) {
        self.flyout = None;
        self.flyout_closed_at = Some(Instant::now());
    }

    /// Brings the main window back (from the flyout).
    pub(crate) fn show_main_window(&mut self, cx: &mut Context<Self>) {
        let main = self.main_window;
        // Deferred: the caller may be inside another window's update.
        cx.defer(move |cx| {
            main.update(cx, |_, window, _| platform::show_window(window)).ok();
        });
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
            Some(i) if i == exit => self.quit(cx),
            _ => {}
        }
    }

    /// Removes the tray icon first: an icon left behind lingers until the user hovers it.
    pub(crate) fn quit(&mut self, cx: &mut Context<Self>) {
        self.tray = None;
        cx.quit();
    }

    pub(crate) fn set_close_to_tray(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.settings.close_to_tray = on;
        self.settings.save();
        cx.notify();
    }

    pub(crate) fn set_lang(&mut self, lang: Lang, cx: &mut Context<Self>) {
        self.lang = lang;
        self.settings.language = Some(lang);
        self.settings.save();
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
}
