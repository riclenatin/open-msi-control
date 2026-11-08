//! Talking to the laptop: connection, polling, writes and simple toggles.

use std::time::Duration;

use gpui::{Context, Task};
use msi_acpi::{HwWorker, MsiLaptop, Scenario, Snapshot};

use super::{Connection, MsiApp};
use crate::i18n::Strings;
use crate::platform;
use crate::telemetry::POLL;

impl MsiApp {
    pub(super) fn on_connected(&mut self, res: msi_acpi::Result<HwWorker>, cx: &mut Context<Self>) {
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
                    this.touchpad = platform::touchpad_enabled();
                    match res {
                        Ok((info, snap)) => {
                            this.info = Some(info);
                            this.history.push(&snap);
                            // A write issued meanwhile makes this reading stale.
                            if this.write_gen == generation {
                                this.snap = Some(snap);
                                this.reconcile(cx);
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
    /// hardware thread, replaces the snapshot with what the EC reports and records it into the
    /// active profile.
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
        self.notice = None;
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
                    Ok(s) if this.write_gen == generation => {
                        this.snap = Some(s);
                        this.capture_profile();
                    }
                    Ok(_) => {}
                    Err(e) => this.error = Some(format!("{}: {e}", what(this.tr())).into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn set_scenario(&mut self, s: Scenario, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_scenario, |snap| snap.scenario = Some(s), move |l| l.set_scenario(s), cx);
    }

    pub(crate) fn set_cooler_boost(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_cooler_boost, |s| s.cooler_boost = Some(on), move |l| l.set_cooler_boost(on), cx);
    }

    pub(crate) fn set_kbd_backlight(&mut self, level: u8, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(
            |t| t.err_backlight,
            |s| s.kbd_backlight = Some(level),
            move |l| l.set_kbd_backlight(level),
            cx,
        );
    }

    pub(crate) fn set_webcam(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_webcam, |s| s.webcam = Some(on), move |l| l.set_webcam(on), cx);
    }

    pub(crate) fn set_win_fn(&mut self, swapped: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(
            |t| t.err_win_fn,
            |s| s.win_fn_swapped = Some(swapped),
            move |l| l.set_win_fn_swap(swapped),
            cx,
        );
    }

    pub(crate) fn set_usb_share(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.act(|t| t.err_usb, |s| s.usb_share = Some(on), move |l| l.set_usb_share(on), cx);
    }

    /// The hotkey is a toggle, so send it and check the registry until the state matches.
    pub(crate) fn set_touchpad(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        if self.touchpad == Some(on) {
            return;
        }
        self.touchpad = Some(on);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mut state = platform::touchpad_enabled();
            for _ in 0..3 {
                if state == Some(on) {
                    break;
                }
                platform::toggle_touchpad();
                cx.background_executor().timer(Duration::from_millis(400)).await;
                state = platform::touchpad_enabled();
            }
            this.update(cx, |this, cx| {
                if state != Some(on) {
                    this.error = Some(this.tr().err_touchpad.into());
                }
                this.touchpad = state;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
