//! Interactive fan curve: a step chart of duty per temperature band with draggable bands.

use gpui::{
    Bounds, Hitbox, HitboxBehavior, PathBuilder, Pixels, Rgba, Window, canvas, fill, linear_color_stop,
    linear_gradient, prelude::*, px,
};
use msi_acpi::CURVE_POINTS;

use super::timeline::dot;
use super::{at, unit};
use crate::theme::theme;
use crate::ui::{DragEvent, Handler, register_drag};

pub const TEMP_MIN: f32 = 30.0;
pub const TEMP_MAX: f32 = 100.0;
const X: (f32, f32) = (TEMP_MIN, TEMP_MAX);
const Y: (f32, f32) = (0.0, 100.0);

#[derive(Clone, Debug)]
pub struct CurveChart {
    pub temps: [u8; CURVE_POINTS],
    pub speeds: [u8; CURVE_POINTS],
    pub top: (u8, u8),
    pub current_temp: Option<u8>,
    pub selected: Option<usize>,
    pub dragging: bool,
}

impl CurveChart {
    /// Horizontal extent of band `i` in unit coordinates; `CURVE_POINTS` is the firmware top band.
    pub fn band(&self, i: usize) -> (f32, f32) {
        let start = if i == 0 {
            0.0
        } else if i == CURVE_POINTS {
            unit(f32::from(self.top.0), X)
        } else {
            unit(f32::from(self.temps[i]), X)
        };
        let end = match i {
            _ if i == CURVE_POINTS => 1.0,
            _ if i + 1 == CURVE_POINTS => unit(f32::from(self.top.0), X),
            _ => unit(f32::from(self.temps[i + 1]), X),
        };
        (start, end)
    }

    pub fn speed(&self, i: usize) -> u8 {
        if i == CURVE_POINTS { self.top.1 } else { self.speeds[i] }
    }

    /// Handle centre of band `i` in unit coordinates (`y` up).
    pub fn handle(&self, i: usize) -> (f32, f32) {
        let (a, b) = self.band(i);
        ((a + b) / 2.0, unit(f32::from(self.speed(i)), Y))
    }

    /// Editable band under unit x, if any.
    pub fn band_at(&self, x: f32) -> Option<usize> {
        (0..CURVE_POINTS).find(|&i| {
            let (a, b) = self.band(i);
            x >= a && x < b
        })
    }

    fn active(&self) -> Option<usize> {
        let temp = self.current_temp?;
        let x = unit(f32::from(temp), X);
        (0..=CURVE_POINTS).find(|&i| {
            let (a, b) = self.band(i);
            x >= a && x < b
        })
    }

    pub fn into_canvas(self, on_drag: Handler<DragEvent>) -> impl IntoElement {
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |bounds, hitbox: Hitbox, window, _| {
                register_drag(window, bounds, &hitbox, self.dragging, on_drag);
                self.paint(bounds, window);
            },
        )
        .size_full()
    }

    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let t = theme();
        let active = self.active();

        if let Some(i) = active {
            let (a, b) = self.band(i);
            let band = Bounds::from_corners(at(bounds, a, 1.0), at(bounds, b, 0.0));
            window.paint_quad(fill(band, t.inset));
        }

        for g in [25.0, 50.0, 75.0] {
            let y = unit(g, Y);
            let mut pb = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(4.)]);
            pb.move_to(at(bounds, 0.0, y));
            pb.line_to(at(bounds, 1.0, y));
            if let Ok(p) = pb.build() {
                window.paint_path(p, t.hairline);
            }
        }

        // Step outline over the editable bands.
        let mut steps = Vec::with_capacity(CURVE_POINTS * 2);
        for i in 0..CURVE_POINTS {
            let (a, b) = self.band(i);
            let y = unit(f32::from(self.speeds[i]), Y);
            steps.push((a, y));
            steps.push((b, y));
        }
        let mut area = PathBuilder::fill();
        area.move_to(at(bounds, 0.0, 0.0));
        for &(x, y) in &steps {
            area.line_to(at(bounds, x, y));
        }
        area.line_to(at(bounds, steps[steps.len() - 1].0, 0.0));
        area.close();
        if let Ok(p) = area.build() {
            let top = Rgba { a: 0.2, ..t.accent };
            let bottom = Rgba { a: 0.02, ..t.accent };
            window.paint_path(
                p,
                linear_gradient(180., linear_color_stop(top, 0.), linear_color_stop(bottom, 1.)),
            );
        }
        let mut line = PathBuilder::stroke(px(2.));
        line.move_to(at(bounds, steps[0].0, steps[0].1));
        for &(x, y) in &steps[1..] {
            line.line_to(at(bounds, x, y));
        }
        // Riser into the firmware band.
        let (top_a, _) = self.band(CURVE_POINTS);
        let top_y = unit(f32::from(self.top.1), Y);
        if let Ok(p) = line.build() {
            window.paint_path(p, t.accent);
        }
        let mut fw = PathBuilder::stroke(px(1.5)).dash_array(&[px(4.), px(3.)]);
        fw.move_to(at(bounds, top_a, steps[steps.len() - 1].1));
        fw.line_to(at(bounds, top_a, top_y));
        fw.line_to(at(bounds, 1.0, top_y));
        if let Ok(p) = fw.build() {
            window.paint_path(p, t.text_faint);
        }

        // Current temperature marker.
        if let Some(temp) = self.current_temp {
            let x = unit(f32::from(temp), X);
            let mut pb = PathBuilder::stroke(px(1.)).dash_array(&[px(3.), px(3.)]);
            pb.move_to(at(bounds, x, 0.0));
            pb.line_to(at(bounds, x, 1.0));
            if let Ok(p) = pb.build() {
                window.paint_path(p, t.warning);
            }
        }

        // Handles.
        for i in 0..=CURVE_POINTS {
            let (x, y) = self.handle(i);
            let c = at(bounds, x, y);
            if i == CURVE_POINTS {
                dot(window, c, 5., t.text_faint);
                dot(window, c, 3., t.card);
            } else if self.selected == Some(i) {
                dot(window, c, 8., t.accent);
                dot(window, c, 3., t.on_accent);
            } else {
                dot(window, c, 6.5, if active == Some(i) { t.warning } else { t.accent });
                dot(window, c, 4., t.card);
            }
        }
    }
}
