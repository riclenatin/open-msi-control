//! Reusable building blocks. Stateless: every component takes values and callbacks; state and
//! transitions live in [`crate::app::MsiApp`].

mod button;
mod drag;
mod pill;
mod row;
mod segmented;
mod surface;
mod switch;
mod text;
mod tooltip;

use std::rc::Rc;

use gpui::{App, Window};

pub use button::{Button, Variant};
pub use drag::{DragEvent, register_drag};
pub use pill::{Tone, pill};
pub use row::control_row;
pub use segmented::{Width, segmented};
pub use surface::{card, icon, inset};
pub use switch::SwitchRow;
pub use text::{mono, tnum};
pub use tooltip::info;

/// Callback shared by components that need to call back into the app from window-level
/// listeners (which must be `'static` and are re-registered every frame).
pub type Handler<E> = Rc<dyn Fn(E, &mut Window, &mut App)>;
