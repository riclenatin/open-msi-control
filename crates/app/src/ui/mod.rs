//! Reusable building blocks. Stateless: every component takes values and callbacks; state and
//! transitions live in [`crate::app::MsiApp`].

mod button;
mod drag;
mod pill;
mod segmented;
mod surface;
mod switch;
mod text;

use std::rc::Rc;

use gpui::{App, Window};

pub use button::{Button, Variant};
pub use drag::{DragEvent, register_drag};
pub use pill::{Tone, pill};
pub use segmented::{Width, segmented};
pub use surface::{card, icon, inset};
pub use switch::switch_row;
pub use text::{mono, tnum};

/// Callback shared by components that need to call back into the app from window-level
/// listeners (which must be `'static` and are re-registered every frame).
pub type Handler<E> = Rc<dyn Fn(E, &mut Window, &mut App)>;
