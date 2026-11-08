//! Remembering what the user set and putting it back after MSI Center overwrites it.
//!
//! Triggers — startup, wake from sleep (a gap between polls) and power-source changes — open a
//! short enforcement window during which any drift from the active profile is re-applied
//! (MSI Center Service rewrites its scenario a few seconds after boot and resume). Outside that
//! window, changes made elsewhere (keyboard hotkeys, MSI Center) are adopted into the profile.

use std::time::{Duration, Instant};

use gpui::Context;

use super::MsiApp;
use crate::platform;
use crate::profile::{Op, Profile, plan};

/// A poll gap longer than this means the machine slept.
const SLEEP_GAP: Duration = Duration::from_secs(10);
/// How long after a trigger drift is treated as an overwrite to undo.
const ENFORCE_WINDOW: Duration = Duration::from_secs(30);
/// Minimum spacing between restore attempts.
const ENFORCE_INTERVAL: Duration = Duration::from_secs(3);

impl MsiApp {
    /// Records the current hardware state into the active profile (after the user changes it).
    pub(super) fn capture_profile(&mut self) {
        let Some(s) = &self.snap else { return };
        let profile = Profile::capture(s);
        let limit = s.battery_limit.unwrap_or(100);
        let on_ac = self.on_ac;
        let changed =
            *self.settings.slot(on_ac) != Some(profile) || self.settings.charge_limit != Some(limit);
        if changed {
            *self.settings.slot(on_ac) = Some(profile);
            self.settings.save();
        }
    }

    /// Called with every fresh reading (no write in flight).
    pub(super) fn reconcile(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let on_ac = platform::on_ac_power();
        let woke = self.last_poll.is_some_and(|t| now.duration_since(t) > SLEEP_GAP);
        let started = self.last_poll.is_none();
        let switched = on_ac != self.on_ac;
        self.last_poll = Some(now);
        self.on_ac = on_ac;
        if started || woke || switched {
            let forced = switched && self.settings.per_power_source;
            self.enforce_until = Some((now + ENFORCE_WINDOW, forced));
        }

        let Some(s) = self.snap.clone() else { return };
        let Some(profile) = *self.settings.slot(on_ac) else {
            // First run (or first time on battery with separate profiles): start from reality.
            self.capture_profile();
            return;
        };
        let ops = plan(&s, &profile, self.settings.charge_limit);
        if ops.is_empty() {
            return;
        }
        let enforcing = self
            .enforce_until
            .is_some_and(|(until, forced)| now < until && (forced || self.settings.restore));
        if !enforcing {
            self.capture_profile();
            return;
        }
        if self.last_enforce.is_some_and(|t| now.duration_since(t) < ENFORCE_INTERVAL) {
            return;
        }
        self.last_enforce = Some(now);
        self.apply_ops(ops, cx);
    }

    fn apply_ops(&mut self, ops: Vec<Op>, cx: &mut Context<Self>) {
        self.act(
            |t| t.err_restore,
            |_| {},
            move |l| {
                for op in ops {
                    match op {
                        Op::Scenario(s) => l.set_scenario(s)?,
                        Op::AutoFan => l.set_auto_fan()?,
                        Op::Curves { cpu, gpu } => l.set_fan_curves(cpu, gpu)?,
                        Op::CoolerBoost(on) => l.set_cooler_boost(on)?,
                        Op::ChargeLimit(pct) => l.set_battery_limit((pct < 100).then_some(pct))?,
                    }
                }
                Ok(())
            },
            cx,
        );
    }

    pub(crate) fn set_restore(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.settings.restore = on;
        self.settings.save();
        cx.notify();
    }

    /// Turning separate profiles on starts the battery profile from the current one.
    pub(crate) fn set_per_source(&mut self, on: bool, keyboard: bool, cx: &mut Context<Self>) {
        self.snap_frame |= keyboard;
        self.settings.per_power_source = on;
        if on && self.settings.battery.is_none() {
            self.settings.battery = self.settings.ac;
        }
        self.settings.save();
        cx.notify();
    }
}
