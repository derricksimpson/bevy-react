//! The core's own elements, registered through the same
//! `add_react_elements` call a feature crate uses ([`CORE_ELEMENTS`]).

pub mod editable;
pub mod image;
pub mod inline_image;
mod node;
mod text;

pub(crate) use node::label_buttons;
pub use node::{BUTTON, DISABLED, DISABLED_WRITER, NODE, ROOT};
pub use text::{TEXT, TEXT_SPAN};

use crate::element::Element;

/// Every core element.
pub static CORE_ELEMENTS: &[&Element] = &[
    &NODE,
    &BUTTON,
    &TEXT,
    &TEXT_SPAN,
    &editable::EDITABLE_TEXT,
    &image::IMAGE,
    &inline_image::INLINE_IMAGE,
    &ROOT,
];
