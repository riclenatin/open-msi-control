//! Battery charge limit: slider preview, debounced writes.

use std::time::Duration;

use gpui::Context;

use super::{Drag, MsiApp};
use crate::ui::DragEvent;

/// Delay before a keyboard-adjusted battery limit is written.
const BATTERY_DEBOUNCE: Duration = Duration::from_millis(450);
pub const BATTERY_MIN: u8 = 40;

impl MsiApp {
    pub(crate) fn battery_value(&self) -> u8 {
        self.battery_preview.or_else(|| self.snap.as_ref().and_then(|s| s.charge_limit)).unwrap_or(100)
    }

    fn write_battery(&mut self, value: u8, cx: &mut Context<Self>) {
        self.battery_write = None;
        self.act(|t| t.err_battery, |s| s.charge_limit = Some(value), move |l| l.set_charge_limit(value), cx);
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
