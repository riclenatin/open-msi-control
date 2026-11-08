//! Motion tokens and interruptible transitions.
//!
//! Rules (Emil Kowalski / tuffui): strong ease-out for things that enter, strong ease-in-out
//! for things that move on screen, UI durations under 300 ms, exits faster than entrances,
//! keyboard-initiated changes are instant, and reduced motion drops movement but keeps fades.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// CSS-style `cubic-bezier(x1, y1, x2, y2)` timing function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    fn sample(a1: f32, a2: f32, t: f32) -> f32 {
        // Bernstein form with P0 = 0, P3 = 1.
        let u = 1.0 - t;
        3.0 * u * u * t * a1 + 3.0 * u * t * t * a2 + t * t * t
    }

    fn slope(a1: f32, a2: f32, t: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * a1 + 6.0 * u * t * (a2 - a1) + 3.0 * t * t * (1.0 - a2)
    }

    /// Progress (0..=1) at normalised time `x` (0..=1).
    pub fn ease(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        if x == 0.0 || x == 1.0 {
            return x;
        }
        // Newton–Raphson, falling back to bisection where the slope is too flat.
        let mut t = x;
        for _ in 0..8 {
            let err = Self::sample(self.x1, self.x2, t) - x;
            if err.abs() < 1e-5 {
                return Self::sample(self.y1, self.y2, t).clamp(0.0, 1.0);
            }
            let d = Self::slope(self.x1, self.x2, t);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        t = x;
        for _ in 0..32 {
            let v = Self::sample(self.x1, self.x2, t);
            if (v - x).abs() < 1e-5 {
                break;
            }
            if v < x {
                lo = t
            } else {
                hi = t
            }
            t = (lo + hi) / 2.0;
        }
        Self::sample(self.y1, self.y2, t).clamp(0.0, 1.0)
    }
}

/// Strong ease-out: entrances, exits, feedback.
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.23, 1.0, 0.32, 1.0);
/// Strong ease-in-out: things moving between two on-screen positions.
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.77, 0.0, 0.175, 1.0);

pub const ENTER: Duration = Duration::from_millis(200);
pub const MOVE: Duration = Duration::from_millis(200);
pub const TOGGLE: Duration = Duration::from_millis(160);
pub const VALUE: Duration = Duration::from_millis(240);
/// Delay between staggered siblings on first appearance.
pub const STAGGER: Duration = Duration::from_millis(45);

/// A transition of position or size. All keyed transitions are movement, so reduced motion
/// makes them instant; fades (first-appearance entrance) use gpui `Animation` and are kept.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub duration: Duration,
    pub curve: CubicBezier,
}

impl Spec {
    /// Moving between two on-screen positions (pills, thumbs, highlights).
    pub const fn movement(duration: Duration) -> Self {
        Self { duration, curve: EASE_IN_OUT }
    }

    /// Easing a displayed value towards a new reading (gauges, slider knob).
    pub const fn value(duration: Duration) -> Self {
        Self { duration, curve: EASE_OUT }
    }
}

#[derive(Clone, Copy, Debug)]
struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    spec: Spec,
}

impl Tween {
    fn settled(value: f32, spec: Spec) -> Self {
        Self { from: value, to: value, start: Instant::now(), spec }
    }

    fn progress(&self, now: Instant) -> f32 {
        if self.spec.duration.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f32() / self.spec.duration.as_secs_f32()).min(1.0)
    }

    fn value(&self, now: Instant) -> f32 {
        let p = self.spec.curve.ease(self.progress(now));
        self.from + (self.to - self.from) * p
    }
}

pub type Key = (&'static str, usize);

/// Keyed, retargetable transitions — CSS transition semantics for immediate-mode UI.
///
/// The first time a key is seen it snaps to its target (nothing animates on first paint).
/// Changing the target mid-flight continues from the current value instead of restarting.
#[derive(Debug, Default)]
pub struct Transitions {
    tweens: HashMap<Key, Tween>,
    /// Snap every change during the current frame (keyboard-initiated input).
    snap: bool,
    reduced_motion: bool,
}

impl Transitions {
    pub fn new(reduced_motion: bool) -> Self {
        Self { reduced_motion, ..Self::default() }
    }

    /// Call at the start of each render.
    pub fn begin_frame(&mut self, snap: bool) {
        self.snap = snap;
    }

    pub fn animate(&mut self, key: Key, target: f32, mut spec: Spec) -> f32 {
        if self.reduced_motion {
            spec.duration = Duration::ZERO;
        }
        let now = Instant::now();
        let snap = self.snap;
        let tween = self.tweens.entry(key).or_insert_with(|| Tween::settled(target, spec));
        if (tween.to - target).abs() > f32::EPSILON {
            *tween = if snap || spec.duration.is_zero() {
                Tween::settled(target, spec)
            } else {
                Tween { from: tween.value(now), to: target, start: now, spec }
            };
        }
        tween.value(now)
    }

    pub fn is_animating(&self) -> bool {
        let now = Instant::now();
        self.tweens.values().any(|t| t.from != t.to && t.progress(now) < 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_matches_css_reference_points() {
        let ease = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
        assert!((ease.ease(0.5) - 0.8024).abs() < 0.002, "{}", ease.ease(0.5));
        for curve in [EASE_OUT, EASE_IN_OUT] {
            assert_eq!(curve.ease(0.0), 0.0);
            assert_eq!(curve.ease(1.0), 1.0);
            let mut prev = 0.0;
            for i in 1..=100 {
                let v = curve.ease(i as f32 / 100.0);
                assert!(v >= prev - 1e-4 && v <= 1.0, "non-monotonic at {i}: {v} < {prev}");
                prev = v;
            }
        }
        // Reference values from an independent bisection solver.
        assert!((EASE_IN_OUT.ease(0.5) - 0.5960).abs() < 0.002, "{}", EASE_IN_OUT.ease(0.5));
        assert!((EASE_OUT.ease(0.5) - 0.9660).abs() < 0.002, "{}", EASE_OUT.ease(0.5));
    }

    #[test]
    fn transitions_snap_first_then_retarget_from_current_value() {
        let mut t = Transitions::new(false);
        let spec = Spec::movement(Duration::from_secs(10));
        assert_eq!(t.animate(("x", 0), 0.0, spec), 0.0);
        assert!(!t.is_animating(), "first sighting must not animate");

        let v = t.animate(("x", 0), 1.0, spec);
        assert!(v < 0.05 && t.is_animating());
        // Interrupt back to 0: continues from where it is, never jumps to the old endpoint.
        let back = t.animate(("x", 0), 0.0, spec);
        assert!(back <= v + 1e-3);
    }

    #[test]
    fn keyboard_frames_and_reduced_motion_are_instant() {
        let spec = Spec::movement(Duration::from_secs(10));
        let mut t = Transitions::new(false);
        t.animate(("x", 0), 0.0, spec);
        t.begin_frame(true);
        assert_eq!(t.animate(("x", 0), 1.0, spec), 1.0);

        let mut reduced = Transitions::new(true);
        reduced.animate(("x", 0), 0.0, spec);
        assert_eq!(reduced.animate(("x", 0), 1.0, spec), 1.0);
    }
}
