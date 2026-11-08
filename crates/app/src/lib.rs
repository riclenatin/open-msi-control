//! open-msi-control: fan, performance and battery control for MSI laptops.
//!
//! The binary (`main.rs`) only calls [`run`]; everything lives here so tests run unelevated.

#[cfg(not(windows))]
compile_error!("open-msi-control is a Windows application (MSI_ACPI is a Windows WMI interface)");

mod app;
mod assets;
mod autostart;
mod charts;
mod dither;
mod i18n;
mod motion;
mod platform;
mod profile;
mod settings;
mod telemetry;
mod theme;
mod tray;
mod ui;
mod update;
mod views;

use std::borrow::Cow;

use gpui::{
    App, Application, Bounds, KeyBinding, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, prelude::*, px, size,
};

use crate::app::{FocusNext, FocusPrev, MsiApp};
use crate::assets::{Assets, FONTS};

/// Starts the application and blocks until it quits. `--tray` (used by autostart) starts with
/// the window hidden in the tray. A second instance just shows the first one's window.
pub fn run() {
    let Some(_instance) = platform::claim_single_instance() else { return };
    let start_hidden = std::env::args().any(|a| a == autostart::TRAY_FLAG);
    Application::new().with_assets(Assets).run(move |cx: &mut App| {
        if let Err(e) = cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()) {
            eprintln!("failed to load bundled fonts, falling back to system UI font: {e}");
        }
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);

        // Narrow, tall window: few settings, one column. Keep clear of the taskbar.
        let screen = cx.primary_display().map(|d| d.bounds().size);
        let fit = |want: f32, avail: Option<gpui::Pixels>, margin: f32| {
            avail.map_or(px(want), |a| px(want).min(a - px(margin)))
        };
        let window_size = size(px(460.), fit(860., screen.map(|s| s.height), 90.));
        let bounds = Bounds::centered(None, window_size, cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Open MSI Control".into()),
                    appears_transparent: true,
                    traffic_light_position: None,
                }),
                window_min_size: Some(size(px(420.), px(520.))),
                window_background: WindowBackgroundAppearance::Opaque,
                show: !start_hidden,
                focus: !start_hidden,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| MsiApp::new(window, cx, start_hidden)),
        );
        if let Err(e) = opened {
            eprintln!("cannot open window: {e}");
            cx.quit();
            return;
        }
        if !start_hidden {
            cx.activate(true);
        }
    });
}
