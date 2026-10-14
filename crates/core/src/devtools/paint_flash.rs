//! Paint flashing: while the panel's "paint" toggle is on, every React node
//! bevy_ui re-extracted this frame flashes a translucent box that fades out —
//! what a state change really repaints (and what the compare-before-write
//! writers and the layer cache save).
//!
//! bevy_ui's extraction is retained: a node's render data is rebuilt only
//! when one of its rendered components changed, and
//! `ExtractedUiNodes::changed` lists exactly those nodes, by main-world
//! entity, for one frame. [`record_repaints`] (render world) copies that set
//! into the shared [`RepaintFeed`]; [`drive_paint_flash`] (main world)
//! drains it the next frame and drives a pool of overlay boxes.
//!
//! The boxes are plain window-root UI nodes above the app (the selection
//! highlight's recipe): writing them never touches an app node, so a flash
//! cannot cause the repaint it reports. They are re-extracted themselves
//! while they fade, but they are not React nodes and never flash.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ComputedUiTargetCamera, UiGlobalTransform};

use crate::bridge::{JsBridge, ReactNode};
use crate::protocol::ROOT_ID;
use crate::react_message;
use crate::reconcile::climb;

use super::DevtoolsState;
use super::pick::highlight_rect;

/// How long a flash takes to fade out, in seconds.
const FADE_SECS: f32 = 0.4;
/// A box's opacity on the frame of the repaint.
const PEAK_ALPHA: f32 = 0.3;

/// JS → Bevy: the panel's "paint" toggle.
#[react_message(name = "devtools.paintFlash")]
pub(super) struct DevtoolsPaintFlashMessage {
    on: bool,
}

pub(super) fn on_paint_flash_message(
    msg: On<DevtoolsPaintFlashMessage>,
    mut state: ResMut<DevtoolsState>,
) {
    state.paint_flash = msg.event().on;
}

/// The render world's report of re-extracted UI nodes, shared by both worlds
/// (a clone in each). A devtools-only path: nothing is recorded, and nothing
/// locked, unless [`Self::on`] is set.
#[derive(Resource, Clone, Default)]
pub(super) struct RepaintFeed(Arc<FeedInner>);

#[derive(Default)]
struct FeedInner {
    /// Whether the render world records (toggle on and panel open).
    on: AtomicBool,
    /// Main-world entities of the nodes re-extracted since the last drain.
    nodes: Mutex<Vec<Entity>>,
}

impl RepaintFeed {
    fn push(&self, nodes: impl Iterator<Item = Entity>) {
        if let Ok(mut feed) = self.0.nodes.lock() {
            feed.extend(nodes);
        }
    }

    fn take(&self) -> Vec<Entity> {
        self.0
            .nodes
            .lock()
            .map(|mut feed| std::mem::take(&mut *feed))
            .unwrap_or_default()
    }
}

/// Render world, after bevy_ui's change scan: hand this frame's re-extracted
/// set to the main world. One atomic load when the toggle is off.
pub(super) fn record_repaints(
    feed: Res<RepaintFeed>,
    // Absent in a render app without bevy's UI renderer.
    extracted: Option<Res<bevy::ui_render::ExtractedUiNodes>>,
) {
    if feed.0.on.load(Ordering::Relaxed)
        && let Some(extracted) = extracted
    {
        feed.push(extracted.changed.iter().map(|main| main.id()));
    }
}

/// Marks a pooled flash box.
#[derive(Component)]
pub(super) struct PaintFlashBox;

/// The flashes in flight and the hidden boxes waiting for reuse.
#[derive(Resource, Default)]
pub(super) struct PaintFlashes {
    /// Flashing node → its box and the seconds since its last repaint.
    live: EntityHashMap<(Entity, f32)>,
    /// Hidden boxes. Never despawned: a box entering or leaving the UI stack
    /// shifts the stack index of everything above it (the panel), which
    /// would re-extract the panel on every flash.
    free: Vec<Entity>,
}

/// Fade the live flashes, then turn the render world's report into boxes.
///
/// A node flashes when it is a React node of the window's UI (the bounds of
/// its border box) — not the panel's own nodes, not a `<surface>`'s (another
/// camera's space), not node-less elements (no box). A node that repaints
/// again while its box fades restarts the fade in place, so one animating
/// every frame holds a steady box that follows it.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_paint_flash(
    state: Res<DevtoolsState>,
    feed: Res<RepaintFeed>,
    // Real time: flashes fade while the Animations tab pauses the app clock.
    time: Res<Time<Real>>,
    bridge: Option<Res<JsBridge>>,
    targets: Query<(&ComputedNode, &UiGlobalTransform), With<ReactNode>>,
    cameras: Query<&ComputedUiTargetCamera>,
    child_of: Query<&ChildOf>,
    flashes: ResMut<PaintFlashes>,
    mut boxes: Query<(&mut Node, &mut BackgroundColor), With<PaintFlashBox>>,
    mut commands: Commands,
) {
    let on = state.open && state.paint_flash;
    if feed.0.on.swap(on, Ordering::Relaxed) && !on {
        // Just switched off: drop what the render world recorded meanwhile.
        feed.take();
    }
    if !on && flashes.live.is_empty() {
        return;
    }
    let PaintFlashes { live, free } = flashes.into_inner();

    // Fade first: a box spawned by the pass below reaches the query once
    // its commands apply, so one missing here was despawned by someone else.
    let dt = time.delta_secs();
    live.retain(|_, (flash_box, age)| {
        let Ok((mut node, mut color)) = boxes.get_mut(*flash_box) else {
            return false;
        };
        *age += dt;
        if *age >= FADE_SECS {
            node.display = Display::None;
            free.push(*flash_box);
            return false;
        }
        color.0 = flash_color(*age);
        true
    });

    let Some(bridge) = bridge.filter(|_| on) else {
        return;
    };
    let camera_of = |e: Entity| cameras.get(e).ok().and_then(ComputedUiTargetCamera::get);
    let window_camera = bridge.nodes.get(&ROOT_ID).copied().and_then(camera_of);
    let panel = state
        .panel_root
        .and_then(|id| bridge.nodes.get(&id).copied());
    let mut repainted = feed.take();
    // Two extracts between drains would list a node twice.
    repainted.sort_unstable();
    repainted.dedup();
    for node in repainted {
        let Ok((computed, transform)) = targets.get(node) else {
            continue;
        };
        if camera_of(node) != window_camera
            || panel.is_some_and(|panel| climb(node, &child_of, |e| e == panel).is_some())
        {
            continue;
        }
        let (pos, size) = highlight_rect(computed, transform);
        let rect = Node {
            position_type: PositionType::Absolute,
            left: Val::Px(pos.x),
            top: Val::Px(pos.y),
            width: Val::Px(size.x),
            height: Val::Px(size.y),
            ..default()
        };
        let peak = BackgroundColor(flash_color(0.0));
        let pooled = live
            .get(&node)
            .map(|&(flash_box, _)| flash_box)
            .or_else(|| free.pop());
        let flash_box = if let Some(flash_box) = pooled
            && let Ok((mut placed, mut color)) = boxes.get_mut(flash_box)
        {
            // `Node` is compared: a write lays the box out again.
            placed.set_if_neq(rect);
            *color = peak;
            flash_box
        } else {
            commands
                .spawn((
                    PaintFlashBox,
                    rect,
                    peak,
                    // Above the app, below the selection highlight
                    // (`i32::MAX - 1`) and the panel (`i32::MAX`).
                    GlobalZIndex(i32::MAX - 2),
                    Pickable::IGNORE,
                ))
                .id()
        };
        live.insert(node, (flash_box, 0.0));
    }
}

/// The flash color `age` seconds after the repaint: green, like a browser's
/// paint flashing, fading linearly to nothing.
fn flash_color(age: f32) -> Color {
    Color::srgba(0.2, 0.9, 0.3, PEAK_ALPHA * (1.0 - age / FADE_SECS).max(0.0))
}

#[cfg(test)]
mod tests;
