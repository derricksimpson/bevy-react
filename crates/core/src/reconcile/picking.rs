//! Pointer state from bevy_picking: what every collector downstream of the
//! hover map builds on.
//!
//! - [`fold_span_hits`] undoes bevy_ui's per-section text targeting for React
//!   text runs before the hover map is built.
//! - [`sync_interactions`] derives [`ReactInteraction`] for every pointer
//!   from the hover map and bevy_picking's press state.
//! - [`sample_pointer`] / [`event_pos`] answer where a pointer is, for the
//!   events the collectors send.

use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::{EntityHashMap, EntityHashSet};
use bevy::ecs::message::MessageMutator;
use bevy::picking::backend::PointerHits;
use bevy::picking::events::PointerState;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{
    Location, PointerButton, PointerId, PointerLocation, PointerPressState,
};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ComputedUiTargetCamera, UiGlobalTransform};

use super::events::climb;
use crate::PointerCapture;
use crate::bridge::{ReactInteraction, ReactNode};
use crate::ext::{EventLocalPos, VirtualPointers};
use crate::layer::pick3d::Transform3dPointer;

/// Retarget a hit on a React text run (a bare-string run or a nested
/// `<text>`) to the `<text>` block that lays it out.
///
/// bevy_ui's picking backend reports the *section* entity under the pointer
/// for a text node. A React run carries no `Pickable` and nothing
/// interactive, so in the hover map it would be a blocking leaf: the block
/// itself, a `<button>` around it and everything else beneath a glyph would
/// drop out of hover. The block's own `Pickable` (the `focusPolicy` mirror)
/// is what the backend already consulted to decide whether to go on, so it
/// is the entity the hover map should see. Text the app spawned itself is
/// left alone.
pub fn fold_span_hits(
    mut hits: MessageMutator<PointerHits>,
    spans: Query<(), (With<TextSpan>, With<ReactNode>)>,
    blocks: Query<(), With<Text>>,
    child_of: Query<&ChildOf>,
) {
    for hits in hits.read() {
        for (entity, _) in &mut hits.picks {
            if spans.contains(*entity)
                && let Some(block) = climb(*entity, &child_of, |e| blocks.contains(e))
            {
                *entity = block;
            }
        }
    }
}

/// The physical device behind a pointer: the mouse and the transform3d
/// pointer (the mouse remapped into a 3D-transformed layer) are one.
pub(crate) fn device(pointer: PointerId, transform3d: Option<&Transform3dPointer>) -> PointerId {
    if transform3d.is_some_and(|t| t.id == pointer) {
        PointerId::Mouse
    } else {
        pointer
    }
}

/// Derive every [`ReactInteraction`] from bevy_picking, for every pointer:
/// an entity in a pointer's [`HoverMap`] entry is `Hovered`; one a pointer's
/// primary button went down on is `Pressed` until that button is released,
/// wherever the pointer is ([`PointerState`]'s `pressing` set).
///
/// Hover is hover-map membership, so it stops at the first blocking node
/// (`focusPolicy: "block"`), exactly like clicks. A disabled control
/// (`InteractionDisabled`) is never `Pressed`, only hovered while the pointer
/// is on it (CSS `:active` / `:hover`). Two pointers' hover is not
/// taken at face value:
/// - the mouse (and its transform3d twin) once the cursor has left its
///   window: bevy_picking keeps the last location, so the map keeps naming
///   what was last under it;
/// - a touch that is no longer down: its pointer lives one more frame, over
///   whatever it was lifted from.
///
/// Also the frame's assigner of [`PointerCapture::over_ui`]: a *window*
/// pointer over or pressing one of these entities. Virtual pointers
/// ([`VirtualPointers`]) don't count — a `<surface>`'s own driver parks its
/// pointer while `over_ui` is set, so counting its hover would oscillate.
/// Later claims (a hovered scrollbar, devtools pick mode) are made in
/// `PointerCaptureSet` and last until the next frame's assignment here.
///
/// The cost is the hover stacks and pressed sets of the live pointers, never
/// the node count. Only entities the two maps name are written, and only
/// what this system set is reset, each compare-before-write — so an idle
/// frame ticks nothing, and a feature writing the component for an entity
/// no picking backend reports is never fought.
#[allow(clippy::too_many_arguments)]
pub fn sync_interactions(
    hover_map: Option<Res<HoverMap>>,
    presses: Option<Res<PointerState>>,
    transform3d: Option<Res<Transform3dPointer>>,
    virtual_pointers: Option<Res<VirtualPointers>>,
    pointers: Query<(&PointerId, &PointerLocation, &PointerPressState)>,
    windows: Query<&Window>,
    mut nodes: Query<&mut ReactInteraction>,
    disabled: Query<(), With<bevy::ui::InteractionDisabled>>,
    // `Option`: headless harnesses run this without the plugin's resources.
    capture: Option<ResMut<PointerCapture>>,
    // What the last run left non-`None`.
    mut live: Local<EntityHashSet>,
    mut next: Local<EntityHashMap<ReactInteraction>>,
) {
    next.clear();
    let mut over_ui = false;
    let cursor_gone = pointers
        .iter()
        .find(|(id, ..)| **id == PointerId::Mouse)
        .and_then(|(_, location, _)| location.location())
        .and_then(|location| match &location.target {
            NormalizedRenderTarget::Window(window) => windows.get(window.entity()).ok(),
            _ => None,
        })
        .is_some_and(|window| window.cursor_position().is_none());
    for (&pointer, _, press) in &pointers {
        let on_window = !virtual_pointers
            .as_ref()
            .is_some_and(|v| v.contains(pointer));
        let hovers = match device(pointer, transform3d.as_deref()) {
            PointerId::Mouse => !cursor_gone,
            PointerId::Touch(_) => press.is_primary_pressed(),
            PointerId::Custom(_) => true,
        };
        if hovers && let Some(hits) = hover_map.as_ref().and_then(|map| map.get(&pointer)) {
            for &entity in hits.keys() {
                if nodes.contains(entity) {
                    next.entry(entity).or_insert(ReactInteraction::Hovered);
                    over_ui |= on_window;
                }
            }
        }
        let pressing = presses
            .as_ref()
            .and_then(|presses| presses.get(pointer, PointerButton::Primary));
        for &entity in pressing.iter().flat_map(|state| state.pressing.keys()) {
            if nodes.contains(entity) {
                if !disabled.contains(entity) {
                    next.insert(entity, ReactInteraction::Pressed);
                }
                over_ui |= on_window;
            }
        }
    }
    for (&entity, &state) in next.iter() {
        if let Ok(mut interaction) = nodes.get_mut(entity) {
            interaction.set_if_neq(state);
        }
    }
    for entity in live.drain() {
        if !next.contains_key(&entity)
            && let Ok(mut interaction) = nodes.get_mut(entity)
        {
            interaction.set_if_neq(ReactInteraction::None);
        }
    }
    live.extend(next.keys().copied());
    if let Some(mut capture) = capture {
        capture.over_ui = over_ui;
    }
}

/// Where a pointer is: its picking location, and the position its events
/// report as `clientX`/`clientY`.
pub(super) struct PointerSample {
    pub location: Location,
    /// Logical px on the pointer's own target. The transform3d pointer sits
    /// at the remapped (layout-space) position, so it reports the real
    /// cursor's instead — like the DOM under a CSS transform.
    pub client: Vec2,
}

/// `None` while the pointer has no location (or no longer exists).
pub(super) fn sample_pointer(
    id: PointerId,
    pointers: &Query<(&PointerId, &PointerLocation)>,
    transform3d: Option<&Transform3dPointer>,
) -> Option<PointerSample> {
    let location = location_of(id, pointers)?.clone();
    let client = client_pos(id, &location, pointers, transform3d);
    Some(PointerSample { location, client })
}

fn location_of<'a>(
    id: PointerId,
    pointers: &'a Query<(&PointerId, &PointerLocation)>,
) -> Option<&'a Location> {
    pointers
        .iter()
        .find(|(pointer, _)| **pointer == id)
        .and_then(|(_, location)| location.location())
}

/// The `clientX`/`clientY` of pointer `id` at `location` (see
/// [`PointerSample::client`]).
pub(super) fn client_pos(
    id: PointerId,
    location: &Location,
    pointers: &Query<(&PointerId, &PointerLocation)>,
    transform3d: Option<&Transform3dPointer>,
) -> Vec2 {
    let device = device(id, transform3d);
    if device == id {
        return location.position;
    }
    location_of(device, pointers).map_or(location.position, |mouse| mouse.position)
}

/// The pointer hovering `entity` this frame (any one, if several do).
pub(super) fn pointer_over(entity: Entity, hover_map: &HoverMap) -> Option<PointerId> {
    hover_map
        .iter()
        .find(|(_, hits)| hits.contains_key(&entity))
        .map(|(pointer, _)| *pointer)
}

/// A node's box, for a position within it. Absent on node-less elements;
/// the target camera only in headless tests.
pub(super) type Geometry = Option<(
    &'static ComputedNode,
    &'static UiGlobalTransform,
    Option<&'static ComputedUiTargetCamera>,
)>;

/// An event's `x`/`y` on an element for a pointer at `location`: the
/// element's own coordinate when its [`EventLocalPos`] slot carries one (an
/// SVG shape's user space), else the clamped `0..1` position within the
/// node's box (top-left origin). `None` when neither is known — a node-less
/// element with an empty slot, a degenerate box.
pub(super) fn event_pos(
    user: Option<&EventLocalPos>,
    geometry: Option<(
        &ComputedNode,
        &UiGlobalTransform,
        Option<&ComputedUiTargetCamera>,
    )>,
    cameras: &Query<&Camera>,
    location: &Location,
) -> Option<Vec2> {
    user.and_then(|user| user.0).or_else(|| {
        let (node, transform, target) = geometry?;
        // Node geometry is in the target's physical px, pointer positions in
        // its logical px.
        let camera = target
            .and_then(|target| target.get())
            .and_then(|camera| cameras.get(camera).ok());
        let physical = match camera {
            Some(camera) => crate::raster::pointer_physical_position(location, camera),
            None => location.position * crate::raster::node_scale_factor(node),
        };
        node.normalize_point(*transform, physical)
            .map(|centered| (centered + 0.5).clamp(Vec2::ZERO, Vec2::ONE))
    })
}

#[cfg(test)]
pub(crate) mod test_pointers;
#[cfg(test)]
mod tests;
