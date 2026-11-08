//! Custom-painted charts. Every chart is a `canvas` sized by its parent; labels that need text
//! are laid out as absolutely positioned divs by the views, using the same scales.

mod curve;
mod gauge;
mod meter;
mod timeline;

pub use curve::{CurveChart, TEMP_MAX, TEMP_MIN};
pub use gauge::tick_gauge;
pub use meter::{bars, segment_meter};
pub use timeline::{TEMP_DOMAIN, TimelineChart};

use gpui::{Bounds, Pixels, Point, point};

/// Maps a value in `domain` onto `0..=1`.
pub fn unit(v: f32, domain: (f32, f32)) -> f32 {
    ((v - domain.0) / (domain.1 - domain.0)).clamp(0.0, 1.0)
}

/// Point inside `bounds` for unit coordinates (`y` up).
pub fn at(bounds: Bounds<Pixels>, x: f32, y: f32) -> Point<Pixels> {
    point(bounds.origin.x + bounds.size.width * x, bounds.origin.y + bounds.size.height * (1.0 - y))
}
