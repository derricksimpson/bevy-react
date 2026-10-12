//! The anchor layer and the per-frame positioning of anchored overlays.

use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use bevy::ui::{IsDefaultUiCamera, UiGlobalTransform, UiTransform, Val2};
use serde::Deserialize;

/// Distance-based scaling config for an anchored overlay. The applied scale is
/// `clamp(1 + factor * (base_distance / distance - 1), min, max)`, so the overlay
/// renders at scale 1 when the camera is exactly `base_distance` away, grows as it
/// gets closer, and shrinks farther out — bounded by `min`/`max`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorScaling {
    /// Lower bound on the applied scale.
    pub min: f32,
    /// Upper bound on the applied scale.
    pub max: f32,
    /// Scaling strength: `0` disables scaling (always 1), `1` is true perspective
    /// (apparent size halves at twice `base_distance`), `2` scales twice as fast.
    pub factor: f32,
    /// Camera distance at which the overlay renders at scale 1.
    pub base_distance: f32,
}

impl AnchorScaling {
    /// Validate a JS-supplied config once at apply time so the per-frame math
    /// can't panic or emit a non-finite scale: any non-finite field disables
    /// scaling (`None`) with a warning, and reversed `min`/`max` bounds are
    /// swapped (a reversed pair would panic `f32::clamp` every frame).
    pub fn sanitized(self) -> Option<Self> {
        if ![self.min, self.max, self.factor, self.base_distance]
            .iter()
            .all(|v| v.is_finite())
        {
            tracing::warn!("non-finite anchor scale config {self:?}; disabling distance scaling");
            return None;
        }
        if self.min > self.max {
            tracing::warn!(
                "anchor scale min {} > max {}; swapping the bounds",
                self.min,
                self.max
            );
            return Some(Self {
                min: self.max,
                max: self.min,
                ..self
            });
        }
        Some(self)
    }
}

/// Distance-based scale for a [sanitized](AnchorScaling::sanitized) config: `1`
/// when the camera is exactly `base_distance` away, growing closer / shrinking
/// farther, pinned to `min..=max`. At `dist == 0` the ratio is `inf`, which
/// `clamp` pins to `max` (closest → largest); a NaN product (`factor == 0` ×
/// `inf`) resolves to `max` for the same reason.
pub(crate) fn distance_scale(c: &AnchorScaling, dist: f32) -> f32 {
    let raw = 1.0 + c.factor * (c.base_distance / dist - 1.0);
    if raw.is_nan() {
        c.max
    } else {
        raw.clamp(c.min, c.max)
    }
}

/// Marker for the dedicated overlay container that every [`Anchored`] node is
/// parented under. Spawned once at startup as its own zero-size UI root at the
/// window origin, sorted below the app's tree (`GlobalZIndex(-1)` — bevy
/// orders equal root keys by which was added or restacked last), so anchored
/// overlays live in their own hierarchy and never contribute to an app
/// container's flex layout or scrollable `content_size`. See
/// [`position_anchored_nodes`].
#[derive(Component, Debug, Clone, Copy)]
pub struct AnchorLayer;

/// Component stamped (by [`ANCHOR_WRITER`](crate::ANCHOR_WRITER)) on every `<anchor>` element. Carries the
/// followed entity, world-space offset, and optional distance scaling. Requires
/// `Visibility` so the system can hide the overlay when its anchor is behind the
/// camera, and `UiTransform` so it can apply the distance scale.
#[derive(Component, Debug, Clone)]
#[require(Visibility, UiTransform)]
pub struct Anchored {
    /// The entity whose world position this overlay follows.
    pub target: Entity,
    /// World-space offset added to the target's translation before projecting.
    pub offset: Vec3,
    /// Distance-based scaling, or `None` to keep the overlay at scale 1.
    pub scale: Option<AnchorScaling>,
}

/// Spawn the [`AnchorLayer`]: its own zero-size UI root at the window
/// origin with default `Overflow::visible`, so it neither clips its children
/// nor intercepts pointer input; anchored nodes position themselves relative
/// to its (0,0) corner. `GlobalZIndex(-1)` sorts it below the app's tree by
/// rule: at the tree's own key the order would be whichever root bevy saw
/// added or restacked last.
pub(crate) fn spawn_anchor_layer(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(0.0),
            height: Val::Px(0.0),
            ..default()
        },
        GlobalZIndex(-1),
        bevy::picking::Pickable::IGNORE,
        // Overlays join bevy's Tab navigation (the app's opt-in) after the
        // window tree's focusables.
        bevy::input_focus::tab_navigation::TabGroup::new(1),
        AnchorLayer,
    ));
}

/// Adopt every [`Anchored`] node: parent it under the shared [`AnchorLayer`]
/// (the element is detached: the bridge never attaches it anywhere), so it
/// lives in its own hierarchy and never contributes to the flex layout or
/// scrollable `content_size` of whatever app container it was declared in,
/// and seed its layout position at the layer's origin.
///
/// Registered in [`ElementOverrideSet`](bevy_react_core::ext::ElementOverrideSet):
/// after the op drain, so a node mounted this frame is under the layer — and
/// a re-render's wholesale `Node` re-stamp is re-seeded — by the time layout
/// runs.
pub(crate) fn adopt_anchored_nodes(
    mut commands: Commands,
    layer: Query<Entity, With<AnchorLayer>>,
    mut anchored: Query<(Entity, Option<&ChildOf>, &mut Node), With<Anchored>>,
) {
    let Ok(layer_entity) = layer.single() else {
        return;
    };
    for (entity, child_of, mut node) in &mut anchored {
        if child_of.map(|c| c.parent()) != Some(layer_entity) {
            commands.entity(entity).insert(ChildOf(layer_entity));
        }
        // Always absolute — so a hidden overlay never takes flex-flow space —
        // at the layer's origin. The node MOVES via `UiTransform.translation`
        // (see `position_anchored_nodes`), never `left`/`top`, which are taffy
        // inputs and would force a full relayout every frame the camera
        // orbits. Guarded so a settled overlay doesn't tick `Changed<Node>`
        // (a relayout) every frame.
        if node.position_type != PositionType::Absolute
            || node.left != Val::Px(0.0)
            || node.top != Val::Px(0.0)
        {
            node.position_type = PositionType::Absolute;
            node.left = Val::Px(0.0);
            node.top = Val::Px(0.0);
        }
    }
}

/// Where `entity` is drawn THIS frame: its `Transform`s composed up the
/// hierarchy. Bevy only propagates them into `GlobalTransform` later in
/// `PostUpdate` — and nothing orders that against UI layout — so the stored
/// `GlobalTransform` is still last frame's here; it answers only for an entity
/// without a `Transform` (one the app writes itself).
fn world_transform(
    entity: Entity,
    transforms: &TransformHelper,
    globals: &Query<&GlobalTransform>,
) -> Option<GlobalTransform> {
    transforms
        .compute_global_transform(entity)
        .ok()
        .or_else(|| globals.get(entity).ok().copied())
}

/// Reposition every [`Anchored`] node each frame: project its target's world
/// position through the UI camera and write the result into the node's
/// `UiTransform.translation`, centered on the anchor point. The node's layout
/// position is a one-time seed ([`adopt_anchored_nodes`]) — movement rides
/// the transform, which is **not** a layout input, so a moving anchor (every
/// frame, while the camera orbits) never re-runs taffy. The trade-off:
/// anchored nodes **own** `translation` — a style/animated `translate` on an
/// `<anchor>` is overwritten each frame (`scale` composes with distance
/// scaling as before; `rotation` is untouched). Hides the overlay until it has
/// been laid out (so it never flashes uncentered on spawn), and when the
/// target has despawned or its anchor point is behind the camera / off-screen.
///
/// Runs in `PostUpdate`, before UI layout (which resolves the translation)
/// and visibility propagation: every `Update` system has moved its cameras
/// and targets by then, and the projection reads this frame's transforms
/// ([`world_transform`]) and camera projection — the overlay is drawn exactly
/// where the 3D frame under it puts the anchor point, never a frame behind a
/// panning camera. An app system that moves a camera or a target in
/// `PostUpdate` orders itself `.before(position_anchored_nodes)`. A no-op when
/// no anchored nodes exist.
#[allow(clippy::type_complexity)]
pub fn position_anchored_nodes(
    default_cam: Query<(Entity, &Camera), With<IsDefaultUiCamera>>,
    other_cam: Query<(Entity, &Camera), Without<IsDefaultUiCamera>>,
    layer: Query<Entity, With<AnchorLayer>>,
    transforms: TransformHelper,
    globals: Query<&GlobalTransform>,
    ui_nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut anchored: Query<(Entity, &Anchored, &mut Visibility, &mut UiTransform)>,
) {
    let Ok(layer_entity) = layer.single() else {
        return;
    };
    // Project through the default UI camera; if none is marked, fall back to any
    // camera (the host app's UI camera may carry no marker).
    let Some((cam_entity, cam)) = default_cam
        .iter()
        .next()
        .or_else(|| other_cam.iter().next())
    else {
        return;
    };
    let Some(cam_tf) = world_transform(cam_entity, &transforms, &globals) else {
        return;
    };
    // Anchored nodes position relative to the layer's box, so subtract its
    // actual top-left (the window origin — the layer is a 0×0 UI root; its
    // transform translation IS its top-left, physical → logical). Pre-first-
    // layout the translation is zero.
    let parent_top_left = ui_nodes
        .get(layer_entity)
        .map(|(c, t)| t.translation * c.inverse_scale_factor())
        .unwrap_or(Vec2::ZERO);

    for (entity, anchor, mut visibility, mut transform) in &mut anchored {
        // Center the overlay on the anchor using its own laid-out size. On the
        // frame it spawns, `bevy_ui` layout hasn't produced a size yet (it runs
        // after this) — so stay hidden one frame rather than flash uncentered.
        let Ok((computed, _)) = ui_nodes.get(entity) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        if computed.size().x <= 0.0 {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }

        // The target may have despawned (or not exist yet): hide until it returns.
        let Some(target_tf) = world_transform(anchor.target, &transforms, &globals) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        let world = target_tf.translation() + anchor.offset;
        let Ok(viewport) = cam.world_to_viewport(&cam_tf, world) else {
            // Behind the camera / outside the viewport: hide rather than clamp.
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };

        // Distance-based scaling (applied via `UiTransform`, which scales about the
        // node center, so the overlay stays centered on its anchor). `None` → 1.
        let scale = match &anchor.scale {
            Some(c) => distance_scale(c, world.distance(cam_tf.translation())),
            None => 1.0,
        };
        if transform.scale != Vec2::splat(scale) {
            transform.scale = Vec2::splat(scale);
        }

        // `world_to_viewport` is in logical pixels, but the node is laid out at
        // the anchor layer's origin — so subtract the layer's top-left
        // (computed once above). Also center this node on the anchor using its
        // own size. Applied as a transform translation (logical px, resolved
        // physical exactly like `left`/`top` would be), compare-guarded so a
        // static camera + target settles.
        let half = computed.size() * computed.inverse_scale_factor() / 2.0;
        let local = viewport - parent_top_left - half;

        let translation = Val2::px(local.x, local.y);
        if transform.translation != translation {
            transform.translation = translation;
        }
        visibility.set_if_neq(Visibility::Inherited);
    }
}
