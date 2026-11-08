//! Design tokens. Dark only: the app lives next to games and dark desktops.
//!
//! Layering follows tuffui: borderless depth, a `card` per section holding `inset` groups;
//! layers differ only by lightness. All colours are authored in OKLCH and contrast-checked in
//! tests below against WCAG 2.2 AA (4.5:1 text, 3:1 large text and UI parts).

mod color;

use std::sync::LazyLock;

use gpui::{Pixels, Rgba, px};

pub use color::{Oklch, oklch};

const ACCENT_HUE: f32 = 162.0;

/// Source definitions; kept separate from the converted [`Theme`] so tests can reason about them.
pub mod palette {
    use super::{ACCENT_HUE, Oklch, oklch};

    pub const BACKGROUND: Oklch = oklch(0.155, 0.0, 0.0);
    pub const CARD: Oklch = oklch(0.2, 0.0, 0.0);
    pub const RAISED: Oklch = oklch(0.265, 0.0, 0.0);
    pub const INSET: Oklch = oklch(1.0, 0.0, 0.0).alpha(0.045);
    pub const HOVER: Oklch = oklch(1.0, 0.0, 0.0).alpha(0.07);
    pub const HAIRLINE: Oklch = oklch(1.0, 0.0, 0.0).alpha(0.07);

    pub const TEXT: Oklch = oklch(0.97, 0.0, 0.0);
    pub const TEXT_MUTED: Oklch = oklch(0.72, 0.004, 286.0);
    pub const TEXT_FAINT: Oklch = oklch(0.64, 0.004, 286.0);

    pub const ACCENT: Oklch = oklch(0.82, 0.15, ACCENT_HUE);
    pub const ACCENT_DIM: Oklch = oklch(0.82, 0.15, ACCENT_HUE).alpha(0.16);
    pub const ON_ACCENT: Oklch = oklch(0.2, 0.03, ACCENT_HUE);
    pub const SECONDARY: Oklch = oklch(0.78, 0.11, 250.0);
    pub const WARNING: Oklch = oklch(0.82, 0.14, 75.0);
    pub const DANGER: Oklch = oklch(0.72, 0.17, 25.0);
    pub const DANGER_DIM: Oklch = oklch(0.72, 0.17, 25.0).alpha(0.16);

    /// Silver primary button (tuffui): light gradient with near-black label.
    pub const SILVER_TOP: Oklch = oklch(1.0, 0.0, 0.0);
    pub const SILVER_BOTTOM: Oklch = oklch(0.76, 0.0, 0.0);
    pub const ON_SILVER: Oklch = oklch(0.2, 0.0, 0.0);
}

#[derive(Debug)]
pub struct Theme {
    pub background: Rgba,
    pub card: Rgba,
    pub raised: Rgba,
    pub inset: Rgba,
    pub hover: Rgba,
    pub hairline: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub text_faint: Rgba,
    pub accent: Rgba,
    pub accent_dim: Rgba,
    pub on_accent: Rgba,
    pub secondary: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub danger_dim: Rgba,
    pub silver_top: Rgba,
    pub silver_bottom: Rgba,
    pub on_silver: Rgba,
    /// Focus ring: the accent, 2px, offset from the control.
    pub focus: Rgba,
}

impl Theme {
    fn new() -> Self {
        use palette as p;
        Self {
            background: p::BACKGROUND.to_rgba(),
            card: p::CARD.to_rgba(),
            raised: p::RAISED.to_rgba(),
            inset: p::INSET.to_rgba(),
            hover: p::HOVER.to_rgba(),
            hairline: p::HAIRLINE.to_rgba(),
            text: p::TEXT.to_rgba(),
            text_muted: p::TEXT_MUTED.to_rgba(),
            text_faint: p::TEXT_FAINT.to_rgba(),
            accent: p::ACCENT.to_rgba(),
            accent_dim: p::ACCENT_DIM.to_rgba(),
            on_accent: p::ON_ACCENT.to_rgba(),
            secondary: p::SECONDARY.to_rgba(),
            warning: p::WARNING.to_rgba(),
            danger: p::DANGER.to_rgba(),
            danger_dim: p::DANGER_DIM.to_rgba(),
            silver_top: p::SILVER_TOP.to_rgba(),
            silver_bottom: p::SILVER_BOTTOM.to_rgba(),
            on_silver: p::ON_SILVER.to_rgba(),
            focus: p::ACCENT.to_rgba(),
        }
    }
}

pub const FONT_SANS: &str = "Geist";
pub const FONT_MONO: &str = "Geist Mono";

/// Corner radii (tuffui scale): card 16, inset/control 12.
pub mod radius {
    use super::{Pixels, px};
    pub fn card() -> Pixels {
        px(16.)
    }
    pub fn control() -> Pixels {
        px(12.)
    }
}

static THEME: LazyLock<Theme> = LazyLock::new(Theme::new);

pub fn theme() -> &'static Theme {
    &THEME
}

#[cfg(test)]
mod tests {
    use super::color::{contrast, over};
    use super::*;

    /// Every background a text token may sit on.
    fn surfaces() -> [(&'static str, Rgba); 4] {
        let t = theme();
        [
            ("background", t.background),
            ("card", t.card),
            ("raised", t.raised),
            ("inset/card", over(t.inset, t.card)),
        ]
    }

    #[track_caller]
    fn assert_contrast(name: &str, fg: Rgba, min: f32) {
        for (bg_name, bg) in surfaces() {
            let ratio = contrast(fg, bg);
            assert!(ratio >= min, "{name} on {bg_name}: {ratio:.2}:1 < {min}:1");
        }
    }

    #[test]
    fn body_text_meets_aa() {
        let t = theme();
        assert_contrast("text", t.text, 4.5);
        assert_contrast("text_muted", t.text_muted, 4.5);
        assert_contrast("text_faint", t.text_faint, 4.5);
        assert_contrast("accent", t.accent, 4.5);
        assert_contrast("secondary", t.secondary, 4.5);
        assert_contrast("warning", t.warning, 4.5);
        assert_contrast("danger", t.danger, 4.5);
    }

    #[test]
    fn ui_parts_meet_non_text_contrast() {
        let t = theme();
        assert_contrast("focus ring", t.focus, 3.0);
    }

    #[test]
    fn filled_controls_keep_label_contrast() {
        let t = theme();
        assert!(contrast(t.on_accent, t.accent) >= 4.5);
        assert!(contrast(t.on_silver, t.silver_bottom) >= 4.5, "label on darkest silver stop");
        // Tinted pills: accent text on its own 16 % tint over a card.
        assert!(contrast(t.accent, over(t.accent_dim, t.card)) >= 4.5);
        assert!(contrast(t.danger, over(t.danger_dim, t.card)) >= 4.5);
    }
}
