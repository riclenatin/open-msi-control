//! Daily update check (opt-out in the App card).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::Context;

use super::MsiApp;
use crate::update;

/// How often the loop wakes to see whether a day has passed since the last check.
const WAKE: Duration = Duration::from_secs(3600);

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl MsiApp {
    pub(super) fn start_update_checks(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let due = this
                    .update(cx, |this, _| {
                        let s = &this.settings;
                        s.check_updates
                            && unix_now().saturating_sub(s.last_update_check) >= update::INTERVAL_SECS
                    })
                    .unwrap_or(false);
                if due {
                    let res = cx.background_executor().spawn(async { update::check() }).await;
                    let alive = this.update(cx, |this, cx| this.on_update_checked(res, cx)).is_ok();
                    if !alive {
                        break;
                    }
                }
                cx.background_executor().timer(WAKE).await;
            }
        })
        .detach();
    }

    fn on_update_checked(&mut self, res: std::io::Result<Option<update::Release>>, cx: &mut Context<Self>) {
        match res {
            Ok(release) => {
                // Only successful checks count: offline at boot means another try within the hour.
                self.settings.last_update_check = unix_now();
                self.settings.save();
                self.update = release;
                cx.notify();
            }
            Err(e) => eprintln!("update check: {e}"),
        }
    }

    pub(crate) fn set_check_updates(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.settings.check_updates = on;
        if !on {
            self.update = None;
        }
        self.settings.save();
        cx.notify();
    }

    pub(crate) fn open_update(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = &self.update {
            cx.open_url(&r.html_url);
        }
    }
}
