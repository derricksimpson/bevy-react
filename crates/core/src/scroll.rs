//! Mouse-wheel scrolling for `overflow: scroll` nodes.
//!
//! `bevy_ui` clips an `OverflowAxis::Scroll` node's content and offsets its
//! children by the node's [`ScrollPosition`], but nothing drives that component
//! from input — Bevy leaves the wheel handling to the app. [`apply_scroll`] is
//! that handler: each frame it reads the accumulated wheel delta, finds the
//! scroll container under the cursor, and moves its `ScrollPosition`.

use bevy::camera::{NormalizedRenderTarget, RenderTarget};
use bevy::ecs::entity::EntityHashSet;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::picking::Pickable;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::prelude::*;
use bevy::ui::{
    CalculatedClip, ComputedNode, ComputedUiTargetCamera, ScrollPosition, UiGlobalTransform,
    UiStack,
};
use bevy::window::PrimaryWindow;

use crate::bridge::{JsBridge, ReactNode, ScrollStep, WheelListener};
use crate::plugin::PointerCapture;
use crate::protocol::{outbound::Outbound, outbound::UiEvent};
use crate::transition::ScrollTransitionState;

/// Default logical pixels scrolled per wheel "line" (mice report `Line` units;
/// trackpads report `Pixel`). A middling value that feels close to a typical
/// desktop. A node's `scrollStep` prop ([`ScrollStep`]) overrides it per container.
const LINE_HEIGHT: f32 = 20.0;

/// Where the wheel points, per UI camera: UI on a window camera is hit-tested
/// against the window cursor; UI on an image-target camera (a `<surface>`)
/// against the [`VirtualPointers`](crate::ext::VirtualPointers) pointer sitting
/// on that image, and is out of reach while none does — its geometry is in
/// texture px, which the window cursor must never be tested against.
#[derive(SystemParam)]
pub struct WheelCursors<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<'w, 's, &'static RenderTarget, With<Camera>>,
    pointers: Query<'w, 's, (&'static PointerId, &'static PointerLocation)>,
    virtual_pointers: Option<Res<'w, crate::ext::VirtualPointers>>,
    target_cameras: Query<'w, 's, &'static ComputedUiTargetCamera>,
    blockers: Query<'w, 's, Blocker>,
    parents: Query<'w, 's, &'static ChildOf>,
}

/// What tells whether a node keeps the wheel from what lies beneath it
/// ([`WheelCursors::blocks`]).
type Blocker = (
    &'static ComputedNode,
    &'static UiGlobalTransform,
    &'static Pickable,
    Option<&'static CalculatedClip>,
    Option<&'static InheritedVisibility>,
);

/// A wheel cursor in physical px (node geometry's space) and logical px (the
/// event's `clientX`/`clientY`).
#[derive(Clone, Copy)]
struct WheelCursor {
    physical: Vec2,
    logical: Vec2,
}

impl WheelCursors<'_, '_> {
    /// The UI nodes the wheel may hit with the cursor each is tested against,
    /// topmost first: window UI before image-target UI (screen UI draws over
    /// the world), each in `UiStack` order.
    fn candidates(&self, ui_stack: &UiStack) -> Vec<(Entity, WheelCursor)> {
        let window = self.windows.single().ok().and_then(|w| {
            let logical = w.cursor_position()?;
            Some(WheelCursor {
                physical: logical * w.scale_factor(),
                logical,
            })
        });
        let on_images: Vec<(AssetId<Image>, WheelCursor)> = self
            .pointers
            .iter()
            .filter(|(id, _)| {
                self.virtual_pointers
                    .as_ref()
                    .is_some_and(|v| v.contains(**id))
            })
            .filter_map(|(_, location)| {
                let location = location.location()?;
                let NormalizedRenderTarget::Image(image) = &location.target else {
                    return None;
                };
                Some((
                    image.handle.id(),
                    WheelCursor {
                        physical: location.position * image.scale_factor,
                        logical: location.position,
                    },
                ))
            })
            .collect();
        // `Some(is_image, cursor)`: the node's cursor, `None` = out of reach.
        let cursor_of = |entity: Entity| -> Option<(bool, WheelCursor)> {
            let camera = self.target_cameras.get(entity).ok().and_then(|c| c.get());
            match camera.and_then(|c| self.cameras.get(c).ok()) {
                Some(RenderTarget::Image(image)) => on_images
                    .iter()
                    .find(|(id, _)| *id == image.handle.id())
                    .map(|(_, cursor)| (true, *cursor)),
                _ => window.map(|cursor| (false, cursor)),
            }
        };
        let mut window_nodes = Vec::new();
        let mut image_nodes = Vec::new();
        for &entity in ui_stack.uinodes.iter().rev() {
            match cursor_of(entity) {
                Some((false, cursor)) => window_nodes.push((entity, cursor)),
                Some((true, cursor)) => image_nodes.push((entity, cursor)),
                None => {}
            }
        }
        window_nodes.extend(image_nodes);
        self.occlude(window_nodes)
    }

    /// Cut `nodes` (topmost first) at the first one under its cursor that
    /// blocks what lies beneath it (`Pickable::should_block_lower` — a
    /// `focusPolicy: "block"` element, the devtools panel): past it only its
    /// ancestors stay. The wheel bubbles up from what it lands on, as a press
    /// does, and never reaches a node a blocking one covers.
    fn occlude(&self, nodes: Vec<(Entity, WheelCursor)>) -> Vec<(Entity, WheelCursor)> {
        let Some(at) = nodes.iter().position(|&(e, c)| self.blocks(e, c)) else {
            return nodes;
        };
        let ancestors: EntityHashSet = self.parents.iter_ancestors(nodes[at].0).collect();
        nodes
            .into_iter()
            .enumerate()
            .filter(|(i, (entity, _))| *i <= at || ancestors.contains(entity))
            .map(|(_, node)| node)
            .collect()
    }

    /// Whether `entity` is hit at `cursor` and blocks what lies beneath it,
    /// tested as bevy's UI picking backend tests a hit (visible, inside its
    /// rect and its clip). A node without a `Pickable` lets the wheel through.
    fn blocks(&self, entity: Entity, cursor: WheelCursor) -> bool {
        self.blockers
            .get(entity)
            .is_ok_and(|(node, transform, pickable, clip, visibility)| {
                pickable.should_block_lower
                    && visibility.is_none_or(|v| v.get())
                    && node.contains_point(*transform, cursor.physical)
                    && clip.is_none_or(|clip| clip.contains_point(cursor.physical))
            })
    }
}

/// Whether a node opts into scrolling on either axis.
pub(crate) fn is_scroll_container(node: &Node) -> bool {
    node.overflow.x == OverflowAxis::Scroll || node.overflow.y == OverflowAxis::Scroll
}

/// The scrollable range in logical px per axis (`0` = "no scroll"). Bevy clamps
/// the offset it *applies* for rendering but never writes that back to
/// `ScrollPosition`, so every scroll writer clamps against this itself.
/// `ComputedNode` sizes are physical; the component is logical — hence
/// `inverse_scale_factor`.
///
/// Mirrors what `ui_layout_system` clamps to, taffy's `Layout::scroll_width`
/// / `scroll_height`: `content_size` is the scrollable overflow extent
/// (measured inside the border), so the range is that extent less the
/// padding box (`size` minus both borders), plus the scrollbar's gutter.
pub(crate) fn scroll_range(computed: &ComputedNode) -> Vec2 {
    let border = computed.border.min_inset + computed.border.max_inset;
    (computed.content_size + computed.scrollbar_size.min(computed.size) - computed.size + border)
        .max(Vec2::ZERO)
        * computed.inverse_scale_factor
}

/// The offset a scroll writer moves from: the eased target when a scroll
/// transition owns the offset (so successive ticks accumulate toward a farther
/// target instead of re-basing mid-ease), else the live position.
pub(crate) fn scroll_base(pos: &ScrollPosition, state: Option<&ScrollTransitionState>) -> Vec2 {
    state.map_or(pos.0, |s| s.target)
}

/// Write `next` to the eased target when a transition owns the offset
/// (`drive_scroll_transition` then moves `ScrollPosition`), else to the live
/// offset directly. `!=`-guarded on both paths so re-writing an unchanged value
/// stays change-tick quiet (`settle_controlled_scroll`'s read-back dedup
/// relies on that).
pub(crate) fn write_scroll(
    pos: &mut ScrollPosition,
    state: Option<Mut<ScrollTransitionState>>,
    next: Vec2,
) {
    match state {
        Some(mut state) => {
            if state.target != next {
                state.target = next;
            }
        }
        None => {
            if pos.0 != next {
                pos.0 = next;
            }
        }
    }
}

/// Move the `ScrollPosition` of the scroll container under the cursor by the
/// frame's wheel delta. Runs in `PointerCaptureSet` after `collect_pointer_events`
/// so that, when it actually scrolls, it can flag the pointer as UI-owned and stop
/// world systems (e.g. a wheel-zoom camera) from also consuming the same wheel.
///
/// The pointer is only claimed (`PointerCapture::over_ui`) when a container actually
/// moves — a scroll container whose content fits (no scroll range on the wheeled axis)
/// lets the wheel fall through, so a camera behind a transparent, non-overflowing
/// scroll pane can still zoom.
///
/// Target selection is by **geometry, topmost-first**. Every `Node` carries a
/// `ScrollPosition` (it's a required component), so "has a `ScrollPosition`" is
/// meaningless — we must check `Node.overflow`. We walk the `UiStack` top to bottom
/// and hit-test each node's rectangle with [`ComputedNode::contains_point`] against
/// its camera's cursor ([`WheelCursors`]: the window cursor, or the in-world
/// pointer on a `<surface>`):
/// - The topmost node under the cursor that declared `onWheel` (a [`WheelListener`])
///   **claims** the wheel: it emits via [`collect_wheel_events`], and here we stop
///   without scrolling anything beneath it. That is what lets an `onWheel` node
///   (e.g. a zoomable map) sit inside a scroll page and swallow the wheel instead of
///   also scrolling its ancestor — DOM `preventDefault` semantics.
/// - Otherwise the topmost real scroll container under the cursor scrolls. This stays
///   content-agnostic: plain nodes on top (text glyphs, images) are transparent
///   because they are neither listeners nor scroll containers.
/// - A node that blocks what lies beneath it (`focusPolicy: "block"`) is opaque:
///   past it only its ancestors are considered ([`WheelCursors`]), so the wheel
///   over a blocking panel scrolls what the panel holds, never what it covers.
///
/// Bevy's layout clamps the *applied* offset to `[0, max]`, so writing freely here
/// never overscrolls.
#[allow(clippy::type_complexity)]
pub fn apply_scroll(
    accumulated: Res<AccumulatedMouseScroll>,
    cursors: WheelCursors,
    ui_stack: Res<UiStack>,
    mut scrollables: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &Node,
        &mut ScrollPosition,
        Option<&ScrollStep>,
        Option<&mut ScrollTransitionState>,
    )>,
    wheel_listeners: Query<(), With<WheelListener>>,
    mut capture: ResMut<PointerCapture>,
) {
    if accumulated.delta == Vec2::ZERO {
        return;
    }
    // Topmost-first: the first node under the cursor that claims the wheel wins.
    // This also resolves nested scroll areas correctly (an inner one sits later in
    // the stack, so it's hit first). Geometry is physical px; a camera with a custom
    // viewport offset is not handled here.
    for (entity, cursor) in cursors.candidates(&ui_stack) {
        let Ok((computed, transform, node, mut pos, step, scroll_state)) =
            scrollables.get_mut(entity)
        else {
            continue;
        };
        if !computed.contains_point(*transform, cursor.physical) {
            continue; // cursor not over this node — transparent, keep scanning
        }
        // An `onWheel` node on top claims the wheel: stop before scrolling anything
        // beneath it. `collect_wheel_events` does the emission (+ the `over_ui`
        // claim); we only need to make sure no ancestor scroll container also moves.
        if wheel_listeners.contains(entity) {
            return;
        }
        if !is_scroll_container(node) {
            continue; // a plain node over the cursor is transparent to scrolling
        }
        // Wheel delta → logical pixels, scaled by this container's step. A positive
        // wheel delta scrolls the view up (content down), i.e. *decreases* the offset.
        let delta = match accumulated.unit {
            MouseScrollUnit::Line => accumulated.delta * step.map(|s| s.0).unwrap_or(LINE_HEIGHT),
            MouseScrollUnit::Pixel => accumulated.delta,
        };
        // Clamp to the scrollable range so the offset can't accumulate past the
        // ends (otherwise you'd have to "unscroll" the slack before it moves).
        let max = scroll_range(computed);
        let base = scroll_base(&pos, scroll_state.as_deref());
        // Only the axes with actual scroll range (`max > 0`) consume the wheel;
        // a container whose content fits has nothing to move.
        let mut next = base;
        let mut consumed = false;
        if node.overflow.x == OverflowAxis::Scroll && delta.x != 0.0 && max.x > 0.0 {
            next.x = (base.x - delta.x).clamp(0.0, max.x);
            consumed = true;
        }
        if node.overflow.y == OverflowAxis::Scroll && delta.y != 0.0 && max.y > 0.0 {
            next.y = (base.y - delta.y).clamp(0.0, max.y);
            consumed = true;
        }
        if consumed {
            write_scroll(&mut pos, scroll_state, next);
            // The wheel consumed an in-range axis — claim the wheel channel
            // so world wheel-consumers that honor `PointerCapture` (the orbit
            // camera's wheel-zoom, etc.) ignore the same wheel this frame. Not
            // a hover claim: `over_ui` stays as this frame's hover left it.
            capture.wheel_captured = true;
            break;
        }
        // Nothing to scroll here (content fits the container) — keep scanning lower
        // containers, and ultimately leave the wheel for world input so e.g. a
        // camera behind a transparent, non-overflowing scroll pane can still zoom.
    }
}

/// A controlled `scrollTop`/`scrollLeft` request (raw, pre-clamp) whose clamp
/// must be redone after layout. The op-apply clamp
/// (`reconcile::update_controlled_scroll`) runs before `UiSystems::Layout`,
/// so when the same commit also changes the content (the canonical case: a
/// log appending rows AND pinning `scrollTop={BIG}` to the bottom), it clamps
/// against LAST frame's `ComputedNode` and lands short — or, worse, the stale
/// clamp equals the live offset and nothing is written at all.
/// [`settle_controlled_scroll`] re-clamps this raw request against fresh
/// post-layout geometry and removes the marker. Inserted whenever a
/// controlled write's request falls outside the stale range (and on create,
/// where no geometry exists yet at all).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct PendingControlledScroll(pub Vec2);

/// Re-clamp deferred controlled-scroll requests against this frame's layout
/// (runs in `PostUpdate` after `UiSystems::Layout`). The corrected offset is
/// applied to child transforms by NEXT frame's layout — one frame late, but
/// exact. The `!=` guards keep change ticks quiet when the stale clamp was
/// already right, so the `onScroll` read-back dedup (which compares against
/// the recorded RAW request) fires exactly once with the real offset when —
/// and only when — the request overshot.
pub fn settle_controlled_scroll(
    mut commands: Commands,
    mut pending: Query<(
        Entity,
        &PendingControlledScroll,
        &ComputedNode,
        &mut ScrollPosition,
        Option<&mut ScrollTransitionState>,
    )>,
) {
    for (entity, request, computed, mut pos, scroll_state) in &mut pending {
        let clamped = request.0.clamp(Vec2::ZERO, scroll_range(computed));
        write_scroll(&mut pos, scroll_state, clamped);
        commands.entity(entity).remove::<PendingControlledScroll>();
    }
}

/// Deliver raw mouse-wheel deltas to the topmost `onWheel` node under the cursor.
///
/// Unlike [`apply_scroll`] (which only moves `overflow: scroll` containers), this
/// fires for **any** node that declared an `onWheel` handler — e.g. a zoomable
/// `<canvas>` map. The delta is passed through untouched (no `ScrollStep` scaling);
/// `deltaMode` (`"line"`/`"pixel"`) tells the app how to read it, like DOM
/// `WheelEvent`.
///
/// Target selection mirrors [`apply_scroll`]: a geometric hit-test over the
/// `UiStack`, topmost first, filtered to [`WheelListener`] nodes and stopped
/// by a blocking node that is not one's ancestor — no hover state needed, so
/// it composes cleanly with the `onPointer*` machinery. Runs in
/// `PointerCaptureSet` after `collect_pointer_events` so the
/// `PointerCapture::wheel_captured` claim survives (that system *resets* it),
/// letting world wheel-consumers (a zoom camera) ignore a wheel a node just
/// handled.
pub fn collect_wheel_events(
    accumulated: Res<AccumulatedMouseScroll>,
    cursors: WheelCursors,
    ui_stack: Res<UiStack>,
    bridge: Res<JsBridge>,
    listeners: Query<(&ComputedNode, &UiGlobalTransform, &ReactNode), With<WheelListener>>,
    mut capture: ResMut<PointerCapture>,
    // `Option` so headless harnesses without input needn't init it.
    keys: Option<Res<ButtonInput<KeyCode>>>,
) {
    if accumulated.delta == Vec2::ZERO {
        return;
    }
    let delta_mode = match accumulated.unit {
        MouseScrollUnit::Line => "line",
        MouseScrollUnit::Pixel => "pixel",
    };

    // Topmost-first: the first `onWheel` node whose rect contains its cursor wins.
    for (entity, cursor) in cursors.candidates(&ui_stack) {
        let Ok((computed, transform, rnode)) = listeners.get(entity) else {
            continue;
        };
        if !computed.contains_point(*transform, cursor.physical) {
            continue;
        }
        // Normalized 0..1 position within the node (top-left origin), matching the
        // pointer events' `x`/`y`. A successful `contains_point` implies a non-zero
        // size, so the divide is safe.
        let size = computed.size;
        let min = transform.translation - size * 0.5;
        let norm = ((cursor.physical - min) / size).clamp(Vec2::ZERO, Vec2::ONE);
        let mut event = UiEvent {
            id: rnode.0,
            kind: "wheel".to_string(),
            x: Some(norm.x),
            y: Some(norm.y),
            client_x: Some(cursor.logical.x),
            client_y: Some(cursor.logical.y),
            delta_x: Some(accumulated.delta.x),
            delta_y: Some(accumulated.delta.y),
            delta_mode: Some(delta_mode.to_string()),
            ..default()
        };
        crate::reconcile::Modifiers::read(keys.as_deref()).stamp(&mut event);
        let _ = bridge.outbound_tx.send(Outbound::UiEvent { event });
        // The node handled the wheel — claim the wheel channel so world
        // wheel-consumers that honor `PointerCapture` ignore the same wheel
        // this frame. Not a hover claim: `over_ui` stays untouched.
        capture.wheel_captured = true;
        break;
    }
}

#[cfg(test)]
mod tests;
