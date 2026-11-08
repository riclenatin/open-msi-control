//! Temperature history: CPU as a solid line with a soft area, GPU dashed (distinguishable
//! without colour), horizontal guides and a hover cursor.

use std::time::Instant;

use gpui::{
    Bounds, ContentMask, Hitbox, HitboxBehavior, MouseMoveEvent, PathBuilder, Pixels, Rgba, Window, canvas,
    fill, linear_color_stop, linear_gradient, prelude::*, px,
};

use super::{at, unit};
use crate::telemetry::{History, Sample, WINDOW};
use crate::theme::theme;
use crate::ui::Handler;

pub const TEMP_DOMAIN: (f32, f32) = (30.0, 100.0);
pub const GUIDES: [f32; 2] = [50.0, 70.0];
/// Above this the lines turn red (same threshold as the header gauges).
pub const TEMP_HOT: f32 = 90.0;

#[derive(Debug)]
pub struct TimelineChart {
    pub points: Vec<(f32, Option<u8>, Option<u8>)>,
    /// Hover position, 0..=1 from the left.
    pub hover: Option<f32>,
}

impl TimelineChart {
    pub fn new(history: &History, hover: Option<f32>) -> Self {
        let now = Instant::now();
        let window = WINDOW.as_secs_f32();
        let points = history
            .samples()
            .iter()
            .map(|s: &Sample| {
                let age = now.duration_since(s.at).as_secs_f32();
                (1.0 - age / window, s.cpu_temp, s.gpu_temp)
            })
            .filter(|p| p.0 >= -0.02)
            .collect();
        Self { points, hover }
    }

    fn series(&self, gpu: bool) -> Vec<(f32, f32)> {
        self.points
            .iter()
            .filter_map(|&(x, c, g)| (if gpu { g } else { c }).map(|t| (x, unit(f32::from(t), TEMP_DOMAIN))))
            .collect()
    }

    /// `on_hover` receives the pointer position (0..=1 from the left) or `None` on leave.
    pub fn into_canvas(self, on_hover: Handler<Option<f32>>) -> impl IntoElement {
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |bounds, hitbox: Hitbox, window, _| {
                let hovering = self.hover.is_some();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    if hitbox.is_hovered(window) {
                        let x = ((e.position.x - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0);
                        on_hover(Some(x), window, cx);
                    } else if hovering {
                        on_hover(None, window, cx);
                    }
                });
                self.paint(bounds, window);
            },
        )
        .size_full()
    }

    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let t = theme();
        let hot_y = unit(TEMP_HOT, TEMP_DOMAIN);
        for (g, color) in
            GUIDES.iter().map(|&g| (g, t.hairline)).chain([(TEMP_HOT, Rgba { a: 0.45, ..t.danger })])
        {
            let y = unit(g, TEMP_DOMAIN);
            let mut pb = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(4.)]);
            pb.move_to(at(bounds, 0.0, y));
            pb.line_to(at(bounds, 1.0, y));
            if let Ok(p) = pb.build() {
                window.paint_path(p, color);
            }
        }

        let cpu = self.series(false);
        if cpu.len() >= 2 {
            let mut area = PathBuilder::fill();
            area.move_to(at(bounds, cpu[0].0, 0.0));
            for &(x, y) in &cpu {
                area.line_to(at(bounds, x, y));
            }
            area.line_to(at(bounds, cpu[cpu.len() - 1].0, 0.0));
            area.close();
            if let Ok(p) = area.build() {
                let top = Rgba { a: 0.22, ..t.accent };
                let bottom = Rgba { a: 0.0, ..t.accent };
                window.paint_path(
                    p,
                    linear_gradient(180., linear_color_stop(top, 0.), linear_color_stop(bottom, 1.)),
                );
            }
            stroke_series(window, bounds, &cpu, px(2.), None, t.accent);
        }
        let gpu = self.series(true);
        if gpu.len() >= 2 {
            stroke_series(window, bounds, &gpu, px(1.5), Some([px(5.), px(4.)]), t.secondary);
        }
        // Above the hot threshold the same lines are repainted red, clipped to that band.
        let hot_band = Bounds::from_corners(at(bounds, 0.0, 1.0), at(bounds, 1.0, hot_y));
        window.with_content_mask(Some(ContentMask { bounds: hot_band }), |window| {
            if cpu.len() >= 2 {
                stroke_series(window, bounds, &cpu, px(2.), None, t.danger);
            }
            if gpu.len() >= 2 {
                stroke_series(window, bounds, &gpu, px(1.5), Some([px(5.), px(4.)]), t.danger);
            }
        });

        // Current-value dots at the right edge.
        let tone = |y: f32, normal: Rgba| if y >= hot_y { t.danger } else { normal };
        for (series, color) in [(&cpu, t.accent), (&gpu, t.secondary)] {
            if let Some(&(x, y)) = series.last() {
                dot(window, at(bounds, x, y), 3.5, tone(y, color));
            }
        }

        if let Some(hx) = self.hover {
            let mut pb = PathBuilder::stroke(px(1.));
            pb.move_to(at(bounds, hx, 0.0));
            pb.line_to(at(bounds, hx, 1.0));
            if let Ok(p) = pb.build() {
                window.paint_path(p, t.text_faint);
            }
            for (series, color) in [(&cpu, t.accent), (&gpu, t.secondary)] {
                if let Some(&(x, y)) =
                    series.iter().min_by(|a, b| (a.0 - hx).abs().total_cmp(&(b.0 - hx).abs()))
                {
                    dot(window, at(bounds, x, y), 4., tone(y, color));
                }
            }
        }
    }
}

fn stroke_series(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    pts: &[(f32, f32)],
    width: Pixels,
    dash: Option<[Pixels; 2]>,
    color: Rgba,
) {
    let mut pb = PathBuilder::stroke(width);
    if let Some(d) = dash {
        pb = pb.dash_array(&d);
    }
    pb.move_to(at(bounds, pts[0].0, pts[0].1));
    for &(x, y) in &pts[1..] {
        pb.line_to(at(bounds, x, y));
    }
    if let Ok(p) = pb.build() {
        window.paint_path(p, color);
    }
}

pub(super) fn dot(window: &mut Window, c: gpui::Point<Pixels>, r: f32, color: Rgba) {
    let b = Bounds::centered_at(c, gpui::size(px(r * 2.), px(r * 2.)));
    window.paint_quad(fill(b, color).corner_radii(px(r)));
}
