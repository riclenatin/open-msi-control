//! OKLCH colour definition and WCAG 2.x contrast math.
//!
//! Tokens are written in OKLCH so lightness steps are perceptually even; they are converted to
//! sRGB once, at startup.

use gpui::Rgba;

/// A colour in OKLCH: `l` 0..=1, `c` chroma (≈0..=0.37), `h` hue in degrees, `alpha` 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
    pub alpha: f32,
}

pub const fn oklch(l: f32, c: f32, h: f32) -> Oklch {
    Oklch { l, c, h, alpha: 1.0 }
}

impl Oklch {
    pub const fn alpha(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }

    /// Gamma-encoded sRGB, clipped to the gamut.
    pub fn to_rgba(self) -> Rgba {
        let (a, b) = {
            let h = self.h.to_radians();
            (self.c * h.cos(), self.c * h.sin())
        };
        let l_ = self.l + 0.396_337_78 * a + 0.215_803_76 * b;
        let m_ = self.l - 0.105_561_346 * a - 0.063_854_17 * b;
        let s_ = self.l - 0.089_484_18 * a - 1.291_485_5 * b;
        let (l, m, s) = (l_.powi(3), m_.powi(3), s_.powi(3));
        let r = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
        let g = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s;
        let bl = -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s;
        Rgba { r: encode(r), g: encode(g), b: encode(bl), a: self.alpha }
    }
}

fn encode(linear: f32) -> f32 {
    let x = linear.clamp(0.0, 1.0);
    if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 }
}

// WCAG math is only needed by the contrast tests that guard the palette.
#[cfg(test)]
fn decode(encoded: f32) -> f32 {
    if encoded <= 0.040_45 { encoded / 12.92 } else { ((encoded + 0.055) / 1.055).powf(2.4) }
}

/// `top` painted over opaque `bottom` (sRGB-space blending, as the renderer does).
#[cfg(test)]
pub fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let mix = |t: f32, b: f32| t * top.a + b * (1.0 - top.a);
    Rgba { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a: 1.0 }
}

/// WCAG relative luminance of an opaque colour.
#[cfg(test)]
pub fn luminance(c: Rgba) -> f32 {
    0.2126 * decode(c.r) + 0.7152 * decode(c.g) + 0.0722 * decode(c.b)
}

/// WCAG contrast ratio (1..=21) of `fg` drawn over opaque `bg`.
#[cfg(test)]
pub fn contrast(fg: Rgba, bg: Rgba) -> f32 {
    let (a, b) = (luminance(over(fg, bg)), luminance(bg));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.004
    }

    #[test]
    fn converts_reference_colours() {
        let white = oklch(1.0, 0.0, 0.0).to_rgba();
        assert!(close(white.r, 1.0) && close(white.g, 1.0) && close(white.b, 1.0));
        // CSS Color 4 reference: oklch(0.628 0.2577 29.23) == #ff0000.
        let red = oklch(0.627_955, 0.257_683, 29.233_885).to_rgba();
        assert!(close(red.r, 1.0) && close(red.g, 0.0) && close(red.b, 0.0), "{red:?}");
    }

    #[test]
    fn contrast_matches_wcag_reference_values() {
        let black = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let white = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        assert!((contrast(white, black) - 21.0).abs() < 0.01);
        // #777777 on white is the classic 4.48:1.
        let grey = Rgba { r: 0x77 as f32 / 255.0, g: 0x77 as f32 / 255.0, b: 0x77 as f32 / 255.0, a: 1.0 };
        assert!((contrast(grey, white) - 4.48).abs() < 0.01);
    }
}
