//! Interior clips — the main-world half of clip-independent layer capture.
//!
//! A promoted layer's capture must not bake **ancestor** clipping (scroll
//! containers / viewport) into its cached pixels: the cache treats scroll as
//! pure translation (see [`super::fold_member_geometry`]), so pixels captured
//! under an ancestor clip would be stale the moment the layer scrolls. Web
//! semantics agree: `filter` applies to the element's full painted content,
//! and ancestor overflow clips the *result* at composite time.
//!
//! [`sync_layer_clips`] therefore re-runs `bevy_ui`'s clip cascade
//! (`update_clipping` semantics: a node's inherited clip is the list of its
//! ancestors' overflow clip rects, each in its clipper's local space; its
//! **own** overflow only clips its children) — but restarted at each layer
//! root with **no inherited clip**. The result
//! is two per-frame maps in [`LayerClips`]:
//!
//! - `interior`: per subtree member, the clip its extracted items should use
//!   *inside the capture* — clips originating inside the subtree still apply
//!   (they move with the layer, so cached pixels stay valid); everything
//!   above the root is stripped. Consumed by the render world's
//!   extract-window swap (`layer::render::clip`), which substitutes these
//!   into the members' [`CalculatedClip`] for exactly the duration of
//!   `ExtractSchedule` — the main world is exclusively borrowed there, so no
//!   main-world system (picking, focus) can ever observe the swap, and every
//!   stock extractor (backgrounds, text, gradients, box shadows, texture
//!   slices) picks the interior clips up uniformly. Text self-clips
//!   (`TextScroll` content boxes) recompose automatically in the stock text
//!   extractors from the swapped value.
//! - `quads`: per layer root, the screen-space rect its **composite quad**
//!   clamps to instead — the bounds ([`clip_aabb`]) of a top-level root's
//!   inherited [`CalculatedClip`], or, for a nested root, of the enclosing
//!   layer's interior cascade value at the root (its quad draws inside the
//!   enclosing capture). A clipper that only translates/scales clips the quad
//!   exactly; a rotated clipper contributes its bounding box.
//!
//! Outside the extract window the real `CalculatedClip` values are always in
//! place: picking (`crate::pick_clip`) and `bevy_ui`'s own consumers keep
//! seeing true inherited clips.
//!
//! **Retained extraction.** `bevy_ui_render` re-extracts a node only when one
//! of its components changed, and the swap deliberately bypasses change
//! ticks. So the clip a node's render data holds is the one in place at its
//! last extraction: a node that joins a layer (promotion) would keep its real
//! ancestor clip baked into the capture, and one that leaves (demotion) its
//! interior clip, drawing outside its scroll container. Every node whose
//! `interior` entry appears, changes or disappears is therefore recorded in
//! [`LayerClips::changed`], and [`mark_reclipped_members`] flags its
//! `CalculatedClip` as changed — re-extracted this frame, inside the swap
//! window. (A real clip change needs no help: bevy's own write flags it.)
//!
//! Known divergence (deliberate, rare): an `OverrideClip` escapee inside a
//! subtree is still clamped by the layer's quad clip at composite time
//! (stock lets it escape every ancestor clip).
//!
//! Offscreen layers still capture (when dirty) like any other — a fully
//! clipped-away quad simply draws nothing. The cost is invisible re-captures
//! for continuously-animated offscreen layers; the win is that scrolling
//! never re-captures and a layer scrolled into view is correct by
//! construction (the bug this module fixes: captures taken under clipping
//! were cached and served stale after scrolling).

use bevy::math::Rect;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui::{CalculatedClip, ComputedNode, FixedNode, OverrideClip, UiGlobalTransform};

use super::{LayerMembership, PromotedLayer};

/// Subtree member → its interior clip ([`LayerClips::interior`]).
pub type InteriorClips = HashMap<Entity, Option<CalculatedClip>>;

/// Clip maps for promoted layers, maintained by [`sync_layer_clips`]. They
/// persist across frames: rebuilt from scratch when [`LayerMembership`] is
/// (a structural frame), refreshed per root when something a cascade depends
/// on changed under it, untouched otherwise — a translating layer without
/// interior clippers costs nothing here.
#[derive(Resource, Debug, Default)]
pub struct LayerClips {
    /// Subtree member → the clip its extracted items use inside the capture
    /// (`None` = unclipped there). Every member gets an entry; the swap only
    /// touches members that carry a real [`CalculatedClip`] — a member whose
    /// interior clip is `Some` always does (its interior clip sources are a
    /// subset of the real cascade's), so no component insert/remove is ever
    /// needed.
    pub interior: InteriorClips,
    /// Promoted root → the screen-space rect its composite quad clamps to.
    /// Top-level roots: their inherited [`CalculatedClip`]. Nested roots: the
    /// enclosing layer's interior cascade value at the root. `None` inside
    /// the option = unclipped.
    pub quads: HashMap<Entity, Option<Rect>>,
    /// Nodes whose `interior` entry appeared, changed or disappeared since
    /// [`mark_reclipped_members`] last drained this (see the module docs on
    /// retained extraction).
    pub changed: Vec<Entity>,
}

/// The screen-space (physical px) bounds of an inherited [`CalculatedClip`]:
/// the intersection of every clip rect's world-space bounding box. `None` =
/// unclipped (no clip rects); a fully clipped node yields an empty rect. Exact
/// for clippers that only translate/scale; a rotated clipper contributes its
/// bounding box (the over-approximation errs on drawing, never on hiding).
pub fn clip_aabb(clip: &CalculatedClip) -> Option<Rect> {
    /// Stands in for a `Visible` axis's infinite clip extent: transforming
    /// an infinite corner multiplies the matrix's zero terms by infinity
    /// (NaN), which `f32::min`/`max` then silently drop.
    const UNBOUNDED: f32 = 1e30;
    let Some(rects) = clip.rects() else {
        return Some(Rect::default());
    };
    rects.iter().fold(None, |bounds: Option<Rect>, c| {
        let to_world = c.world_to_clip_local.inverse();
        let r = Rect {
            min: c.rect.min.max(Vec2::splat(-UNBOUNDED)),
            max: c.rect.max.min(Vec2::splat(UNBOUNDED)),
        };
        let world = [
            r.min,
            Vec2::new(r.max.x, r.min.y),
            Vec2::new(r.min.x, r.max.y),
            r.max,
        ]
        .into_iter()
        .fold(Rect::EMPTY, |acc, p| {
            acc.union_point(to_world.transform_point2(p))
        });
        Some(bounds.map_or(world, |b| b.intersect(world)))
    })
}

/// A [`CalculatedClip`] of one screen-space rect (an identity
/// `world_to_clip_local`) — how tests spell a plain clip rect.
#[cfg(test)]
pub(crate) fn screen_clip(rect: Rect) -> CalculatedClip {
    let mut clip = CalculatedClip::default();
    clip.push_rect(rect, bevy::math::Affine2::IDENTITY);
    clip
}

/// What [`sync_layer_clips`] last consumed from [`LayerMembership`]'s
/// counters: a moved `rebuilds` means the maps are stale wholesale, a moved
/// `syncs` means `clip_touched` is this frame's.
#[derive(Default)]
pub struct ClipSyncMemory {
    rebuilds: u64,
    syncs: u64,
}

/// Maintains [`LayerClips`]. Runs in `PostUpdate` after
/// [`super::sync_layer_geometry`] (this frame's [`LayerMembership`]) and after
/// `bevy_ui`'s `UiSystems::PostLayout` (final [`CalculatedClip`] values for
/// the top-level quad clips). Must NOT feed [`super::resolve_layer_repaints`]:
/// clip changes never dirty a capture — that is the point.
///
/// A cascade's inputs are the subtree's hierarchy, each member's `Node`
/// (overflow, display, clip margin), `OverrideClip` presence, the geometry of
/// **clipping** members only, and — for a top-level root's quad — the root's
/// own `CalculatedClip`. Each has a signal: a membership rebuild re-cascades
/// everything; otherwise only the roots owning a changed input re-cascade
/// (the geometry sync hands over the clipper-geometry roots in
/// `LayerMembership::clip_touched`, the rest are queried here). Scrollbar
/// widgets are excluded from the `Node` signal for the ping-pong reason the
/// geometry gate documents.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn sync_layer_clips(
    roots: Query<(Entity, Option<&CalculatedClip>), With<PromotedLayer>>,
    root_markers: Query<(), With<PromotedLayer>>,
    children: Query<&Children>,
    nodes: Query<(
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        Has<OverrideClip>,
        Has<FixedNode>,
    )>,
    changed_nodes: Query<
        Entity,
        (
            Changed<Node>,
            Without<bevy::ui_widgets::Scrollbar>,
            Without<bevy::ui_widgets::ScrollbarThumb>,
        ),
    >,
    changed_root_clips: Query<Entity, (Changed<CalculatedClip>, With<PromotedLayer>)>,
    mut removed_clips: RemovedComponents<CalculatedClip>,
    added_overrides: Query<Entity, Added<OverrideClip>>,
    mut removed_overrides: RemovedComponents<OverrideClip>,
    membership: Res<LayerMembership>,
    mut memory: Local<ClipSyncMemory>,
    mut clips: ResMut<LayerClips>,
) {
    let cascade_root = |root: Entity,
                        calculated_clip: Option<&CalculatedClip>,
                        prev: Option<&InteriorClips>,
                        clips: &mut LayerClips| {
        // Only ACTIVE roots (present in this frame's membership as their own
        // layer) ran `mark_subtree`; inactive ones (zero-sized, not laid out)
        // have no capture to clip and may carry stale components.
        if membership.node_to_layer.get(&root) != Some(&root) {
            return;
        }
        // Interior cascade: restart with NO inherited clip at the root —
        // exactly `bevy_ui`'s `update_clipping`, minus everything above.
        cascade(
            root,
            root,
            None,
            &children,
            &root_markers,
            &nodes,
            prev,
            clips,
        );
        // A top-level root's quad composites into the screen phase and clamps
        // to the root's real inherited clip. (Nested roots' quad clips were
        // recorded by their enclosing root's cascade at the prune point.)
        if membership.enclosing.get(&root) == Some(&None) {
            clips
                .quads
                .insert(root, calculated_clip.and_then(clip_aabb));
        }
    };
    if memory.rebuilds != membership.rebuilds {
        // Structural frame: membership is fresh, the maps are stale wholesale.
        memory.rebuilds = membership.rebuilds;
        memory.syncs = membership.syncs;
        removed_clips.clear();
        removed_overrides.clear();
        let clips = &mut *clips;
        // Rebuilt into a fresh map, diffed against the old one: only the
        // entries that really changed are re-extracted.
        let prev = std::mem::take(&mut clips.interior);
        clips.quads.clear();
        for (root, calculated_clip) in &roots {
            cascade_root(root, calculated_clip, Some(&prev), clips);
        }
        let LayerClips {
            interior, changed, ..
        } = clips;
        changed.extend(prev.keys().filter(|e| !interior.contains_key(*e)));
        return;
    }
    let mut touched: Vec<Entity> = Vec::new();
    if memory.syncs != membership.syncs {
        memory.syncs = membership.syncs;
        touched.extend_from_slice(&membership.clip_touched);
    }
    let to_root = |entity: Entity| {
        if root_markers.contains(entity) {
            Some(entity)
        } else {
            membership.node_to_layer.get(&entity).copied()
        }
    };
    touched.extend(changed_nodes.iter().filter_map(to_root));
    touched.extend(added_overrides.iter().filter_map(to_root));
    touched.extend(removed_overrides.read().filter_map(to_root));
    touched.extend(&changed_root_clips);
    touched.extend(removed_clips.read().filter(|e| root_markers.contains(*e)));
    if touched.is_empty() {
        return; // Clip-idle: last frame's maps stand (no `ResMut` deref).
    }
    touched.sort_unstable();
    touched.dedup();
    let clips = &mut *clips;
    for root in touched {
        if let Ok((root, calculated_clip)) = roots.get(root) {
            cascade_root(root, calculated_clip, None, clips);
        }
    }
}

/// Flag the `CalculatedClip` of every node whose interior clip entry changed
/// ([`LayerClips::changed`]) so `bevy_ui_render`'s retained extraction
/// re-extracts it this frame, inside the swap window. Runs right after
/// [`sync_layer_clips`]. Entities without the component (unclipped, or
/// despawned) need nothing: there is no clip in their render data to refresh.
pub fn mark_reclipped_members(
    mut clips: ResMut<LayerClips>,
    mut clipped: Query<&mut CalculatedClip>,
) {
    if clips.changed.is_empty() {
        return; // No `ResMut` deref: the maps stay unchanged-flagged.
    }
    for entity in clips.changed.drain(..) {
        if let Ok(mut clip) = clipped.get_mut(entity) {
            clip.set_changed();
        }
    }
}

/// One step of the interior clip cascade — mirrors `bevy_ui`'s
/// `update_clipping` recursion (`bevy_ui-0.20.0/src/layout/clipping.rs`):
/// `OverrideClip`/`FixedNode` reset the inherited clip, `Display::None` fully
/// clips, the node records its *inherited* value, and its own overflow only
/// affects its children via [`children_clip`]. A nested promoted root is
/// pruned: the cascade value at the prune point is its composite quad's clip
/// (its quad draws inside this capture), and its interior belongs to its own
/// cascade.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn cascade(
    node: Entity,
    dfs_root: Entity,
    mut inherited: Option<CalculatedClip>,
    children: &Query<&Children>,
    root_markers: &Query<(), With<PromotedLayer>>,
    nodes: &Query<(
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        Has<OverrideClip>,
        Has<FixedNode>,
    )>,
    prev: Option<&InteriorClips>,
    clips: &mut LayerClips,
) {
    let Ok((ui_node, computed, transform, has_override, has_fixed)) = nodes.get(node) else {
        return; // Non-UI entity: nothing to clip, nothing extracted below it.
    };
    if has_override || has_fixed {
        inherited = None;
    }
    if ui_node.display == bevy::ui::Display::None {
        inherited = Some(CalculatedClip::FullyClipped);
    }
    if node != dfs_root && root_markers.contains(node) {
        clips
            .quads
            .insert(node, inherited.as_ref().and_then(clip_aabb));
        return; // The nested root's own cascade owns its interior.
    }
    let child_clip = children_clip(ui_node, computed, transform, &inherited);
    // The previous value: the old map on a rebuild, else this map's entry.
    if prev.unwrap_or(&clips.interior).get(&node) != Some(&inherited) {
        clips.changed.push(node);
    }
    clips.interior.insert(node, inherited);
    if let Ok(kids) = children.get(node) {
        for &kid in kids {
            cascade(
                kid,
                dfs_root,
                child_clip.clone(),
                children,
                root_markers,
                nodes,
                prev,
                clips,
            );
        }
    }
}

/// The clip a node passes to its children — stock semantics: `Visible`
/// overflow (or a fully clipped node) passes the inherited clip through;
/// otherwise the node's [`ComputedNode::resolve_clip_rect`] (object-centered,
/// physical px) is appended in the node's local space, and a node without an
/// invertible transform fully clips its children.
fn children_clip(
    node: &Node,
    computed: &ComputedNode,
    transform: &UiGlobalTransform,
    inherited: &Option<CalculatedClip>,
) -> Option<CalculatedClip> {
    if inherited
        .as_ref()
        .is_some_and(CalculatedClip::is_fully_clipped)
        || node.overflow.is_visible()
    {
        return inherited.clone();
    }
    let Some(clip_from_world) = transform.try_inverse() else {
        return Some(CalculatedClip::FullyClipped);
    };
    let mut clip = inherited.clone().unwrap_or_default();
    clip.push_rect(
        computed.resolve_clip_rect(node.overflow, node.overflow_clip_margin),
        clip_from_world,
    );
    Some(clip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::{
        LayerContentDirt, LayerGroupAlpha, LayerRepaintState, LayersRegistry, PromotionReasons,
        resolve_layer_repaints, sync_layer_geometry,
    };
    use crate::protocol::NodeId;
    use bevy::math::Affine2;
    use bevy::ui::Overflow;

    /// World + schedule harness like `layer::tests::geometry_world`, extended
    /// with [`sync_layer_clips`] between geometry and repaint resolution —
    /// the production ordering.
    fn clip_world() -> (World, Schedule) {
        let mut world = World::new();
        world.init_resource::<LayerMembership>();
        world.init_resource::<LayersRegistry>();
        world.init_resource::<LayerRepaintState>();
        world.init_resource::<LayerContentDirt>();
        world.init_resource::<LayerClips>();
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                sync_layer_geometry,
                sync_layer_clips,
                resolve_layer_repaints,
            )
                .chain(),
        );
        (world, schedule)
    }

    /// A laid-out plain UI node: `center` is the `UiGlobalTransform`
    /// translation (node center), so the border box is `center ± size/2`.
    fn spawn_node(world: &mut World, size: Vec2, center: Vec2, overflow: Overflow) -> Entity {
        world
            .spawn((
                Node {
                    overflow,
                    ..Default::default()
                },
                ComputedNode {
                    size,
                    ..Default::default()
                },
                UiGlobalTransform::from(Affine2::from_translation(center)),
            ))
            .id()
    }

    /// A promoted layer root (adds the marker components
    /// [`sync_layer_geometry`] requires on top of [`spawn_node`]).
    fn spawn_root(
        world: &mut World,
        id: NodeId,
        size: Vec2,
        center: Vec2,
        overflow: Overflow,
    ) -> Entity {
        let e = spawn_node(world, size, center, overflow);
        world.entity_mut(e).insert((
            crate::bridge::ReactNode(id),
            LayerGroupAlpha(1.0),
            PromotedLayer {
                reasons: PromotionReasons(PromotionReasons::FILTER),
            },
        ));
        e
    }

    /// A member's interior clip as screen-space bounds (`None` = unclipped).
    fn interior(world: &World, e: Entity) -> Option<Rect> {
        world
            .resource::<LayerClips>()
            .interior
            .get(&e)
            .unwrap_or_else(|| panic!("no interior clip entry for {e:?}"))
            .as_ref()
            .and_then(clip_aabb)
    }

    fn quad(world: &World, e: Entity) -> Option<Rect> {
        *world
            .resource::<LayerClips>()
            .quads
            .get(&e)
            .unwrap_or_else(|| panic!("no quad clip entry for {e:?}"))
    }

    /// Ancestor clipping (hand-inserted `CalculatedClip`, as real `bevy_ui`
    /// would under a scrollport) is STRIPPED from every member's interior
    /// clip — and becomes the top-level root's quad clip instead. A root
    /// without any inherited clip gets an unclipped quad.
    #[test]
    fn ancestor_clip_stripped_and_moved_to_quad() {
        let (mut world, mut schedule) = clip_world();
        let clip = Rect::new(0.0, 0.0, 30.0, 30.0);
        let root = spawn_root(
            &mut world,
            1,
            Vec2::new(100.0, 60.0),
            Vec2::new(50.0, 30.0),
            Overflow::visible(),
        );
        let child = spawn_node(
            &mut world,
            Vec2::new(40.0, 20.0),
            Vec2::new(50.0, 30.0),
            Overflow::visible(),
        );
        world.entity_mut(child).insert(ChildOf(root));
        world.entity_mut(root).insert(screen_clip(clip));
        world.entity_mut(child).insert(screen_clip(clip));
        let unclipped = spawn_root(
            &mut world,
            2,
            Vec2::new(50.0, 50.0),
            Vec2::new(300.0, 300.0),
            Overflow::visible(),
        );
        schedule.run(&mut world);

        assert_eq!(interior(&world, root), None);
        assert_eq!(interior(&world, child), None);
        assert_eq!(quad(&world, root), Some(clip));
        assert_eq!(quad(&world, unclipped), None);
    }

    /// The root's own overflow clips its CHILDREN only (stock semantics):
    /// the root's paint is unclipped in-capture, children clamp to the root's
    /// `resolve_clip_rect`, and deeper interior clippers intersect.
    #[test]
    fn root_overflow_clips_children_only() {
        let (mut world, mut schedule) = clip_world();
        let root = spawn_root(
            &mut world,
            1,
            Vec2::splat(200.0),
            Vec2::splat(100.0),
            Overflow::clip(),
        );
        // Offset so the container's clip rect escapes the root's on the right.
        let container = spawn_node(
            &mut world,
            Vec2::splat(100.0),
            Vec2::new(160.0, 100.0),
            Overflow::clip(),
        );
        world.entity_mut(container).insert(ChildOf(root));
        let grandchild = spawn_node(
            &mut world,
            Vec2::splat(40.0),
            Vec2::new(160.0, 100.0),
            Overflow::visible(),
        );
        world.entity_mut(grandchild).insert(ChildOf(container));
        schedule.run(&mut world);

        let root_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
        let container_rect = Rect::new(110.0, 50.0, 210.0, 150.0);
        assert_eq!(interior(&world, root), None, "root paint unclipped");
        assert_eq!(interior(&world, container), Some(root_rect));
        assert_eq!(
            interior(&world, grandchild),
            Some(root_rect.intersect(container_rect)),
            "interior clippers intersect"
        );
    }

    /// A nested promoted root is PRUNED from the enclosing DFS: the cascade
    /// value at the prune point becomes its quad clip, and its own interior
    /// cascade restarts from nothing — outer clips must not leak in.
    #[test]
    fn nested_roots_get_quad_clips_and_restart_interior() {
        let (mut world, mut schedule) = clip_world();
        let outer = spawn_root(
            &mut world,
            1,
            Vec2::splat(300.0),
            Vec2::splat(150.0),
            Overflow::visible(),
        );
        let clipper = spawn_node(
            &mut world,
            Vec2::splat(100.0),
            Vec2::splat(100.0),
            Overflow::clip(),
        );
        world.entity_mut(clipper).insert(ChildOf(outer));
        let inner = spawn_root(
            &mut world,
            2,
            Vec2::splat(80.0),
            Vec2::splat(100.0),
            Overflow::visible(),
        );
        world.entity_mut(inner).insert(ChildOf(clipper));
        let inner_child = spawn_node(
            &mut world,
            Vec2::splat(40.0),
            Vec2::splat(100.0),
            Overflow::visible(),
        );
        world.entity_mut(inner_child).insert(ChildOf(inner));
        // Deeper: a clipper partially OUTSIDE the outer clipper's rect, then
        // a third root — its quad clip must be the inner cascade value alone
        // (restart proof: no intersection with the outer clipper's rect).
        let clipper2 = spawn_node(
            &mut world,
            Vec2::splat(60.0),
            Vec2::new(140.0, 100.0),
            Overflow::clip(),
        );
        world.entity_mut(clipper2).insert(ChildOf(inner));
        let innermost = spawn_root(
            &mut world,
            3,
            Vec2::splat(40.0),
            Vec2::new(140.0, 100.0),
            Overflow::visible(),
        );
        world.entity_mut(innermost).insert(ChildOf(clipper2));
        schedule.run(&mut world);

        let clipper_rect = Rect::new(50.0, 50.0, 150.0, 150.0);
        let clipper2_rect = Rect::new(110.0, 70.0, 170.0, 130.0);
        assert_eq!(quad(&world, inner), Some(clipper_rect));
        assert_eq!(interior(&world, inner), None, "interior restarts");
        assert_eq!(interior(&world, inner_child), None);
        assert_eq!(
            quad(&world, innermost),
            Some(clipper2_rect),
            "inner cascade only — outer clipper must not leak through the restart"
        );
        assert_eq!(interior(&world, clipper), None, "outer member");
    }

    /// `OverrideClip` resets the inherited interior clip; `Display::None`
    /// forces an empty clip down its subtree (stock `update_clipping` order).
    #[test]
    fn override_clip_and_display_none() {
        let (mut world, mut schedule) = clip_world();
        let root = spawn_root(
            &mut world,
            1,
            Vec2::splat(200.0),
            Vec2::splat(100.0),
            Overflow::clip(),
        );
        let root_rect = Rect::new(0.0, 0.0, 200.0, 200.0);

        let escapee = spawn_node(
            &mut world,
            Vec2::splat(40.0),
            Vec2::splat(100.0),
            Overflow::visible(),
        );
        world
            .entity_mut(escapee)
            .insert((ChildOf(root), OverrideClip));
        let escapee_child = spawn_node(
            &mut world,
            Vec2::splat(20.0),
            Vec2::splat(100.0),
            Overflow::visible(),
        );
        world.entity_mut(escapee_child).insert(ChildOf(escapee));

        let hidden = spawn_node(
            &mut world,
            Vec2::splat(40.0),
            Vec2::splat(100.0),
            Overflow::visible(),
        );
        world.entity_mut(hidden).insert(ChildOf(root));
        world.entity_mut(hidden).get_mut::<Node>().unwrap().display = bevy::ui::Display::None;
        let hidden_child = spawn_node(
            &mut world,
            Vec2::splat(20.0),
            Vec2::splat(100.0),
            Overflow::visible(),
        );
        world.entity_mut(hidden_child).insert(ChildOf(hidden));

        let plain = spawn_node(
            &mut world,
            Vec2::splat(40.0),
            Vec2::splat(100.0),
            Overflow::visible(),
        );
        world.entity_mut(plain).insert(ChildOf(root));
        schedule.run(&mut world);

        assert_eq!(interior(&world, escapee), None);
        assert_eq!(interior(&world, escapee_child), None);
        assert_eq!(interior(&world, hidden), Some(Rect::default()));
        assert_eq!(interior(&world, hidden_child), Some(Rect::default()));
        assert_eq!(interior(&world, plain), Some(root_rect));
    }

    /// THE regression the whole change exists for: scrolling (uniform
    /// translation of the subtree under a fixed ancestor clip) must keep the
    /// capture cache clean — clips never feed the repaint resolver — while
    /// the quad clip stays live; and a clip-rect change alone must not dirty
    /// either.
    #[test]
    fn scroll_holds_cache_and_updates_quad_clip() {
        let (mut world, mut schedule) = clip_world();
        let clip = Rect::new(0.0, 0.0, 200.0, 100.0);
        // Content sits below a 100-tall scrollport (fully clipped away).
        let root = spawn_root(
            &mut world,
            1,
            Vec2::new(100.0, 60.0),
            Vec2::new(50.0, 130.0),
            Overflow::visible(),
        );
        let child = spawn_node(
            &mut world,
            Vec2::new(40.0, 20.0),
            Vec2::new(50.0, 130.0),
            Overflow::visible(),
        );
        world.entity_mut(child).insert(ChildOf(root));
        world.entity_mut(root).insert(screen_clip(clip));
        world.entity_mut(child).insert(screen_clip(clip));
        schedule.run(&mut world);
        schedule.run(&mut world); // settle: steady state is clean
        assert!(world.resource::<LayerRepaintState>().dirty.is_empty());
        assert_eq!(quad(&world, root), Some(clip));

        // Scroll into view: the whole subtree translates; the clip stays.
        let delta = Vec2::new(0.0, -100.0);
        for e in [root, child] {
            let center = world.get::<UiGlobalTransform>(e).unwrap().translation + delta;
            *world.get_mut::<UiGlobalTransform>(e).unwrap() =
                UiGlobalTransform::from(Affine2::from_translation(center));
        }
        schedule.run(&mut world);
        assert!(
            world.resource::<LayerRepaintState>().dirty.is_empty(),
            "scrolling must not re-capture"
        );
        assert_eq!(quad(&world, root), Some(clip));

        // The scrollport itself changes: quad clip follows, still no dirt.
        let grown = Rect::new(0.0, 0.0, 200.0, 150.0);
        *world.get_mut::<CalculatedClip>(root).unwrap() = screen_clip(grown);
        schedule.run(&mut world);
        assert!(
            world.resource::<LayerRepaintState>().dirty.is_empty(),
            "a clip change must not re-capture"
        );
        assert_eq!(quad(&world, root), Some(grown));
    }

    /// Retained extraction keeps the clip of a node's last extraction in its
    /// render data, so a membership transition must flag the member's
    /// `CalculatedClip` (re-extracted inside the swap window) — joining a
    /// layer and leaving it — while an idle frame flags nothing.
    #[test]
    fn membership_transitions_flag_clips_for_reextraction() {
        let (mut world, mut schedule) = clip_world();
        schedule.add_systems(mark_reclipped_members.after(sync_layer_clips));
        let flagged = |world: &World, e: Entity| {
            world
                .entity(e)
                .get_ref::<CalculatedClip>()
                .unwrap()
                .is_changed()
        };
        let clip = Rect::new(0.0, 0.0, 30.0, 30.0);
        let root = spawn_root(
            &mut world,
            1,
            Vec2::new(100.0, 60.0),
            Vec2::new(50.0, 30.0),
            Overflow::visible(),
        );
        let child = spawn_node(
            &mut world,
            Vec2::new(40.0, 20.0),
            Vec2::new(50.0, 30.0),
            Overflow::visible(),
        );
        world.entity_mut(child).insert(ChildOf(root));
        world.entity_mut(root).insert(screen_clip(clip));
        world.entity_mut(child).insert(screen_clip(clip));
        schedule.run(&mut world);
        assert!(world.resource::<LayerClips>().changed.is_empty(), "drained");

        // Idle: nothing joins or leaves, nothing is flagged.
        world.clear_trackers();
        schedule.run(&mut world);
        assert!(!flagged(&world, root) && !flagged(&world, child), "idle");

        // Demotion: the members' render data holds the interior clip.
        world.clear_trackers();
        world.entity_mut(root).remove::<PromotedLayer>();
        schedule.run(&mut world);
        assert!(world.resource::<LayerClips>().interior.is_empty());
        assert!(flagged(&world, root) && flagged(&world, child), "demoted");

        // Promotion: the members' render data holds the real ancestor clip.
        world.clear_trackers();
        world.entity_mut(root).insert(PromotedLayer {
            reasons: PromotionReasons(PromotionReasons::FILTER),
        });
        schedule.run(&mut world);
        assert!(flagged(&world, root) && flagged(&world, child), "promoted");
    }

    /// A scroll container clips one axis only: bevy_ui gives its `Visible`
    /// axis an infinite extent, which must stay unbounded in the screen
    /// bounds (transforming it naively yields NaN, i.e. an empty rect that
    /// clips a nested layer's quad away entirely).
    #[test]
    fn single_axis_clip_bounds_stay_unbounded_on_the_open_axis() {
        let (mut world, mut schedule) = clip_world();
        let outer = spawn_root(
            &mut world,
            1,
            Vec2::splat(300.0),
            Vec2::splat(150.0),
            Overflow::visible(),
        );
        let scroller = spawn_node(
            &mut world,
            Vec2::new(200.0, 100.0),
            Vec2::new(150.0, 100.0),
            Overflow::scroll_y(),
        );
        world.entity_mut(scroller).insert(ChildOf(outer));
        let inner = spawn_root(
            &mut world,
            2,
            Vec2::splat(40.0),
            Vec2::new(150.0, 100.0),
            Overflow::visible(),
        );
        world.entity_mut(inner).insert(ChildOf(scroller));
        schedule.run(&mut world);

        let clip = quad(&world, inner).expect("the scroller clips the nested quad");
        assert_eq!((clip.min.y, clip.max.y), (50.0, 150.0), "the clipped axis");
        assert!(
            clip.min.x < -1e20 && clip.max.x > 1e20,
            "the open axis stays unbounded, got {clip:?}"
        );
        assert!(
            crate::layer::render::clip::clip_quad(
                Vec2::new(130.0, 80.0),
                bevy::math::UVec2::splat(40),
                0.0,
                Some(clip)
            )
            .is_some(),
            "the nested quad still draws"
        );
    }
}
