//! The `<anchor>` element for `bevy_react_core`: world-anchored UI overlays.
//!
//! An anchored element is an ordinary screen-space `bevy_ui` node, but each
//! frame its on-screen position is recomputed by projecting a target entity's
//! world position (plus an optional offset) through the UI camera. That is how
//! floating labels, nameplates, and health bars track a 3D entity while staying
//! flat, fully interactive overlays — no render-to-texture, no second camera,
//! no synthetic-pointer picking (clicks ride the window pointer's normal path).
//!
//! Apps normally get this crate through `bevy-react`'s `anchor` cargo feature
//! (on by default): `bevy_react::anchor`, with [`AnchorPlugin`] a `ReactPlugins`
//! member. Depending on this crate directly, add [`AnchorPlugin`] to an app that
//! also has `ReactUiPlugin` (any order):
//!
//! ```no_run
//! use bevy::prelude::*;
//! use bevy_react_anchor::AnchorPlugin;
//! use bevy_react_core::ReactUiPlugin;
//!
//! App::new()
//!     .add_plugins(DefaultPlugins)
//!     .add_plugins((ReactUiPlugin::new("ui/dist/app.js"), AnchorPlugin))
//!     .run();
//! ```
//!
//! Without the plugin an `<anchor>` in the React tree mounts as a plain node
//! and the bridge reports a `featureMissing` warning naming its feature and plugin.
//!
//! Everything here plugs into the core through its element registry
//! ([`bevy_react_core::element`]) and extension contract
//! ([`bevy_react_core::ext`]):
//!
//! - [`ANCHOR`] is a registered [`Element`] with three attributes —
//!   [`ENTITY`] (the followed entity's `Entity::to_bits()`, required),
//!   [`OFFSET`] (a world-space offset), and [`SCALE`] (distance scaling,
//!   [`AnchorScaling`]) — and one writer ([`ANCHOR_WRITER`]) turning them into
//!   the [`Anchored`] binding.
//! - The element is **detached**: the core never attaches it under its React
//!   parent (removing that parent still despawns it); the crate parents it
//!   under the [`AnchorLayer`] — its own zero-size UI root, sorted below the
//!   app's tree — so an overlay never takes part in its declared parent's
//!   flex layout or scroll range. That adoption runs in the contract's
//!   [`ElementOverrideSet`](bevy_react_core::ext::ElementOverrideSet), after
//!   the op drain.
//! - [`position_anchored_nodes`] runs in `PostUpdate`, before UI layout: it
//!   projects through this frame's camera and target transforms, so an
//!   overlay is drawn where the 3D frame under it puts its anchor point — not
//!   a frame behind a moving camera — and its `UiTransform.translation` wins
//!   over whatever an `Update` animation or transition wrote.
//!
//! [`Element`]: bevy_react_core::element::Element

use bevy::prelude::*;
use bevy_react_core::ReactAppExt;

mod element;
mod position;

#[cfg(test)]
mod tests;

pub use element::{ANCHOR, ANCHOR_WRITER, ENTITY, OFFSET, SCALE};
pub use position::{AnchorLayer, AnchorScaling, Anchored, position_anchored_nodes};

/// Registers the `<anchor>` element, the [`AnchorLayer`], and the per-frame
/// positioning. Requires `ReactUiPlugin` in the same app (added in any order;
/// warned at `finish` when missing).
pub struct AnchorPlugin;

impl Plugin for AnchorPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_systems(Startup, position::spawn_anchor_layer)
            .add_systems(
                Update,
                position::adopt_anchored_nodes.in_set(bevy_react_core::ext::ElementOverrideSet),
            )
            .add_systems(
                PostUpdate,
                position_anchored_nodes
                    .after(bevy::camera::CameraUpdateSystems)
                    .before(bevy::ui::UiSystems::Layout)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
            );
    }

    fn finish(&self, app: &mut App) {
        bevy_react_core::ext::warn_without_core::<Self>(app, "anchor");
    }
}

/// The element registration alone (no systems) — what the TypeScript
/// exporter and a headless op harness need.
pub fn register_bindings(app: &mut App) {
    app.add_react_element(&ANCHOR);
}
