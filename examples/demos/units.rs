//! The Units page's "Root font size" control: React sets Bevy's [`RemSize`],
//! the size every `rem` — and, on an element with no font, every `em` — is a
//! multiple of. bevy-react adds no API over the resource; a message is all
//! it takes.

use bevy::prelude::*;
use bevy_react::{ReactAppExt, react_message};

/// React → Bevy: set the root font size, in logical px
/// (`bevy.units.setRemSize(px)`).
#[react_message(name = "units.setRemSize")]
pub struct SetRemSize(pub f32);

/// Register the handler (shared by the live app and the `--export-bindings`
/// exporter).
pub fn register_bindings(app: &mut App) {
    app.add_react_handler(set_rem_size);
}

fn set_rem_size(on: On<SetRemSize>, mut rem: ResMut<RemSize>) {
    // Compare first: a write lays the whole UI out again.
    rem.set_if_neq(RemSize(on.event().0.clamp(8.0, 48.0)));
}
