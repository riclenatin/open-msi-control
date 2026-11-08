use std::sync::Arc;

use gpui::{Div, FontFeatures, SharedString, Styled, div, prelude::*};

use crate::theme::FONT_MONO;

/// Tabular figures, so changing numbers do not jitter horizontally.
pub fn tnum<E: Styled>(mut el: E) -> E {
    el.text_style().get_or_insert_with(Default::default).font_features =
        Some(FontFeatures(Arc::new(vec![("tnum".into(), 1)])));
    el
}

pub fn mono(text: impl Into<SharedString>) -> Div {
    div().font_family(FONT_MONO).child(text.into())
}
