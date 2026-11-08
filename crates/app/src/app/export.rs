//! Saving the sensor history as CSV.

use std::path::PathBuf;
use std::time::SystemTime;

use gpui::Context;

use super::MsiApp;
use crate::telemetry::rfc3339;

impl MsiApp {
    /// Asks for a file name (suggested in Documents) and writes the whole history there.
    pub(crate) fn export_csv(&mut self, cx: &mut Context<Self>) {
        let dir =
            std::env::var_os("USERPROFILE").map_or_else(PathBuf::new, |p| PathBuf::from(p).join("Documents"));
        // `2026-11-08T12:34:56Z` → `20261108-123456`
        let stamp: String = rfc3339(SystemTime::now()).chars().filter(char::is_ascii_digit).collect();
        let name = format!("open-msi-control-{}-{}.csv", &stamp[..8], &stamp[8..]);
        let csv = self.history.to_csv();
        let picked = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else { return };
            let written = cx.background_executor().spawn({
                let path = path.clone();
                async move { std::fs::write(&path, csv) }
            });
            let res = written.await;
            this.update(cx, |this, cx| {
                match res {
                    Ok(()) => this.notice = Some((this.tr().exported)(&path.display().to_string()).into()),
                    Err(e) => this.error = Some(format!("{}: {e}", this.tr().err_export).into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
