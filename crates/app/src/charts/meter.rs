//! Small inline meters: segmented duty bar and a bar sparkline.

use gpui::{Div, Rgba, div, prelude::*, px};

use crate::theme::theme;

/// `cells` segments, lit up to `fraction` (0..=1).
pub fn segment_meter(fraction: f32, cells: usize, color: Rgba) -> Div {
    let t = theme();
    let lit = (fraction.clamp(0.0, 1.0) * cells as f32).round() as usize;
    div().flex().items_center().gap(px(2.)).children(
        (0..cells)
            .map(|i| div().w(px(3.)).h(px(12.)).rounded(px(1.)).bg(if i < lit { color } else { t.raised })),
    )
}

/// Bar sparkline of recent values scaled to `max`; the newest bar is highlighted.
pub fn bars(values: &[u32], max: u32, color: Rgba) -> Div {
    let t = theme();
    let max = max.max(1) as f32;
    let n = values.len();
    div().flex().items_end().gap(px(2.)).h(px(18.)).children(values.iter().enumerate().map(|(i, &v)| {
        let h = (v as f32 / max).clamp(0.08, 1.0) * 18.0;
        div().w(px(3.)).h(px(h)).rounded(px(1.)).bg(if i + 1 == n { color } else { t.raised })
    }))
}
