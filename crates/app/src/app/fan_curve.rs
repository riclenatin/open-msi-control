//! Fan-curve editing: drafts, dragging, applying.

use gpui::Context;
use msi_acpi::Fan;

use super::{Curves, Drag, MsiApp};
use crate::charts::CurveChart;
use crate::ui::DragEvent;

impl MsiApp {
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

    pub(crate) fn on_curve_drag(&mut self, e: DragEvent, chart: &CurveChart, cx: &mut Context<Self>) {
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
}
