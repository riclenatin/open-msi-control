//! Pointer dragging for custom-painted controls (slider, fan curve).
//!
//! gpui's element `on_mouse_move` only fires while hovered, so a drag that leaves the element
//! would stall. Instead we register window-level listeners during paint: press is hover-gated,
//! move/release are not (Windows captures the mouse on button down).

use gpui::{
    Bounds, CursorStyle, Hitbox, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Window, point,
};

use crate::ui::Handler;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragEvent {
    /// Pointer pressed; position normalised to the bounds (0..=1 each axis, y down).
    Press(Point<f32>),
    Move(Point<f32>),
    Release,
}

fn normalise(bounds: Bounds<Pixels>, p: Point<Pixels>) -> Point<f32> {
    let local = p - bounds.origin;
    point((local.x / bounds.size.width).clamp(0.0, 1.0), (local.y / bounds.size.height).clamp(0.0, 1.0))
}

/// Call from a canvas paint closure. `dragging` is the app's current drag state for this control.
pub fn register_drag(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    hitbox: &Hitbox,
    dragging: bool,
    handler: Handler<DragEvent>,
) {
    if dragging {
        window.set_window_cursor_style(CursorStyle::ClosedHand);
    } else {
        window.set_cursor_style(CursorStyle::PointingHand, hitbox);
    }
    let press = handler.clone();
    let hitbox = hitbox.clone();
    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
        if phase.bubble() && e.button == MouseButton::Left && hitbox.is_hovered(window) {
            press(DragEvent::Press(normalise(bounds, e.position)), window, cx);
        }
    });
    if !dragging {
        return;
    }
    let mv = handler.clone();
    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
        if phase.capture() && e.dragging() {
            mv(DragEvent::Move(normalise(bounds, e.position)), window, cx);
        }
    });
    window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
        if phase.capture() && e.button == MouseButton::Left {
            handler(DragEvent::Release, window, cx);
        }
    });
}
