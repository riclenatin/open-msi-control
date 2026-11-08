//! Procedural header art: a planet horizon with an atmospheric glow, posterised with an 8×8
//! Bayer ordered dither. Generated once at startup; purely decorative.

use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};

use crate::theme::{Oklch, oklch, palette};

pub const WIDTH: u32 = 1000;
pub const HEIGHT: u32 = 300;
/// Image pixels per dither cell.
const CELL: u32 = 2;

const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Sky glow: background → MSI red → amber → warm white.
const WARM: [Oklch; 6] = [
    palette::BACKGROUND,
    oklch(0.24, 0.06, 20.0),
    oklch(0.42, 0.16, 25.0),
    oklch(0.58, 0.21, 30.0),
    oklch(0.76, 0.16, 58.0),
    oklch(0.95, 0.05, 85.0),
];

/// Planet limb: background → indigo → electric blue.
const COOL: [Oklch; 5] = [
    palette::BACKGROUND,
    oklch(0.21, 0.05, 265.0),
    oklch(0.32, 0.12, 262.0),
    oklch(0.48, 0.17, 255.0),
    oklch(0.68, 0.14, 240.0),
];

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Cheap integer hash for sparse "stars".
fn hash(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

fn pick(ramp: &[[u8; 4]], intensity: f32, threshold: f32) -> [u8; 4] {
    let levels = (ramp.len() - 1) as f32;
    let idx = (intensity.clamp(0.0, 1.0) * levels + threshold).floor() as usize;
    ramp[idx.min(ramp.len() - 1)]
}

fn bgra(ramp: &[Oklch]) -> Vec<[u8; 4]> {
    ramp.iter()
        .map(|c| {
            let c = c.to_rgba();
            let q = |v: f32| (v * 255.0).round() as u8;
            [q(c.b), q(c.g), q(c.r), 255]
        })
        .collect()
}

pub fn render() -> Arc<RenderImage> {
    let (w, h) = (WIDTH as f32, HEIGHT as f32);
    let warm = bgra(&WARM);
    let cool = bgra(&COOL);
    // Planet: huge circle whose top crests at 62 % of the height, right of centre.
    let (cx, cy) = (w * 0.62, h * 2.6);
    let radius = cy - h * 0.6;

    let mut img = RgbaImage::new(WIDTH, HEIGHT);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (gx, gy) = (x / CELL, y / CELL);
        let threshold = (f32::from(BAYER[(gy % 8) as usize][(gx % 8) as usize]) + 0.5) / 64.0;
        // Sample at the cell centre so each dither cell is a flat square.
        let (fx, fy) = ((gx * CELL) as f32 + 0.5, (gy * CELL) as f32 + 0.5);
        let (dx, dy) = (fx - cx, fy - cy);
        let d = (dx * dx + dy * dy).sqrt() - radius;
        // Brightest slightly right of the crest, fading along the limb.
        let theta = dx.atan2(-dy);
        let strength = (-((theta - 0.12) / 0.42).powi(2)).exp();
        // Keep the left third dark so header text keeps its contrast.
        let mask = smoothstep(0.18, 0.48, fx / w);

        let colour = if d >= 0.0 {
            let glow = (-d / 46.0).exp() * strength;
            let rim = (-(d / 3.0).powi(2)).exp() * strength;
            let haze = (-d / 150.0).exp() * strength * 0.35;
            let star = f32::from(u8::from(hash(gx, gy).is_multiple_of(900))) * 0.22 * (1.0 - fy / h);
            pick(&warm, ((glow * 0.85 + rim + haze) * mask).max(star), threshold)
        } else {
            let limb = (d / 70.0).exp() * strength;
            pick(&cool, limb * 0.9 * mask, threshold)
        };
        *px = image::Rgba(colour);
    }
    Arc::new(RenderImage::new(vec![Frame::new(img)]))
}
