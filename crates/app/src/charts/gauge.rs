//! Tick gauge: a 240° arc of ticks, lit up to the value, coloured by threshold.

use std::f32::consts::PI;

use gpui::{PathBuilder, Rgba, canvas, point, prelude::*, px};

use super::unit;
use crate::theme::theme;

const TICKS: usize = 36;
const SWEEP: f32 = 240.0;

/// `value` already animated by the caller; `None` draws an idle gauge.
pub fn tick_gauge(value: Option<f32>, domain: (f32, f32), warn: f32, hot: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let t = theme();
            let c = bounds.center();
            let r_out = bounds.size.width.min(bounds.size.height) / 2.0 - px(2.);
            let r_in = r_out - px(9.);
            let lit = value.map_or(0, |v| (unit(v, domain) * TICKS as f32).round() as usize);
            let tone = |v: f32| -> Rgba {
                if v >= hot {
                    t.danger
                } else if v >= warn {
                    t.warning
                } else {
                    t.accent
                }
            };
            // Group ticks by colour: one path per colour keeps draw calls low.
            let mut groups: [(Rgba, PathBuilder); 4] = [
                (t.raised, PathBuilder::stroke(px(2.))),
                (t.accent, PathBuilder::stroke(px(2.))),
                (t.warning, PathBuilder::stroke(px(2.))),
                (t.danger, PathBuilder::stroke(px(2.))),
            ];
            for i in 0..TICKS {
                let f = i as f32 / (TICKS - 1) as f32;
                let angle = (90.0 + SWEEP / 2.0 - f * SWEEP) * PI / 180.0;
                let (s, co) = angle.sin_cos();
                let a = point(c.x + r_in * co, c.y - r_in * s);
                let b = point(c.x + r_out * co, c.y - r_out * s);
                let color = if i < lit { tone(domain.0 + f * (domain.1 - domain.0)) } else { t.raised };
                if let Some((_, pb)) = groups.iter_mut().find(|(gc, _)| *gc == color) {
                    pb.move_to(a);
                    pb.line_to(b);
                }
            }
            for (color, pb) in groups {
                if let Ok(p) = pb.build() {
                    window.paint_path(p, color);
                }
            }
        },
    )
    .size_full()
}
