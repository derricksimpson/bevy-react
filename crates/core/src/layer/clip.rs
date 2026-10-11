//! Interior clips — the main-world half of clip-independent layer capture.
//!
//! A promoted layer's capture must not bake **ancestor** clipping (scroll
//! containers / viewport) into its cached pixels: the cache treats scroll as
//! pure translation (see [`super::fold_member_geometry`]), so pixels captured
//! under an ancestor clip would be stale the moment the layer scrolls. Web
//! semantics agree: `filter` applies to the element's full painted content,
//! and ancestor overflow clips the *result* at composite time.
//!
//! `bevy_ui` hands every node its **inherited** clip as a list
//! ([`CalculatedClip`]): one rect per clipping ancestor, outermost first,
//! each in its clipper's local space; a node's own overflow is appended only
//! to the list its children get (`bevy_ui`'s `layout/clipping.rs`). So with
//! no reset in between, a member's list is its layer root's list plus the
//! clippers from the root down — and the clip the member needs *inside the
//! capture* is its own list minus that leading part. [`sync_layer_clips`]
//! keeps exactly that in [`LayerClips`]:
//!
//! - `strip`: per subtree member, how many leading rects of its real clip
//!   come from above its layer root ([`inherited_len`]). Only members with at
//!   least one have a row. Consumed by the render world's extract-window swap
//!   (`layer::render::clip`), which drops those rects from the members'
//!   [`CalculatedClip`] for exactly the duration of `ExtractSchedule` — the
//!   main world is exclusively borrowed there, so no main-world system
//!   (picking, focus) can ever observe the swap, and every stock extractor
//!   (backgrounds, text, gradients, box shadows, texture slices, UI
//!   materials) picks the interior clips up uniformly. What stays is Bevy's
//!   own rects, untouched: clips originating inside the subtree still apply
//!   (they move with the layer, so cached pixels stay valid), rotated or
//!   scaled clippers included. Text self-clips (`TextScroll` content boxes)
//!   recompose automatically in the stock text extractors from the swapped
//!   value.
//! - `quads`: per layer root, what clips its **composite quad** instead
//!   ([`QuadClip`]) — a top-level root's inherited [`CalculatedClip`], or,
//!   for a nested root, its list minus the enclosing root's (its quad draws
//!   inside the enclosing capture). Clippers that only translate/scale (the
//!   common case, a layout transition's FLIP scale included) reduce to one
//!   screen-space rect the quad clamps to; a rotated clipper keeps the list,
//!   and the quad is cut to its polygon.
//!
//! Outside the extract window the real `CalculatedClip` values are always in
//! place: picking and `bevy_ui`'s other consumers keep seeing true inherited
//! clips.
//!
//! **What the strip relies on.** Bevy's list being append-only and outermost
//! first, and a child's leading rects being clones of the values its ancestor
//! got in the same pass — so an equal prefix is recognized with plain
//! equality (the tests in `clip/tests.rs` run Bevy's own clipping system and
//! fail if either changes). Where the lists differ the member keeps its whole
//! clip:
//!
//! - An `OverrideClip`/`FixedNode` between the root and the member restarts
//!   the member's list. (The one unsound corner: a clipper below such a reset
//!   whose rect and transform equal, bit for bit, a clipper above the root
//!   would be taken for it — the capture is then under-clipped by a rect the
//!   quad clamp applies anyway. bevy-react inserts neither component.)
//! - A fully clipped member (`display: none`, a clipper with a singular
//!   transform) stays fully clipped. When the cause sits **above** the root
//!   the whole subtree extracts fully clipped and the capture is empty; it is
//!   never shown (the quad is clipped away too), and the cause lifting
//!   changes the root's size or linear part, which re-captures.
//!
//! **Retained extraction.** `bevy_ui_render` re-extracts a node only when one
//! of its components changed, and the swap deliberately bypasses change
//! ticks. So the clip a node's render data holds is the one in place at its
//! last extraction: a node that joins a layer under a scrollport (promotion)
//! would keep its real ancestor clip baked into the capture, and one that
//! leaves (demotion) its interior clip, drawing outside its scroll container.
//! Every node whose `strip` row appears, changes or disappears is therefore
//! recorded in [`LayerClips::changed`], and [`mark_reclipped_members`] flags
//! its `CalculatedClip` as changed — re-extracted this frame, inside the swap
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

use std::sync::Arc;

use bevy::ecs::entity::EntityHashMap;
use bevy::math::Rect;
use bevy::prelude::*;
use bevy::ui::CalculatedClip;

use super::LayerMembership;

/// Clip maps for promoted layers, maintained by [`sync_layer_clips`]. They
/// persist across frames: re-derived for every member when
/// [`LayerMembership`] is rebuilt (a structural frame), per node whose
/// `CalculatedClip` changed otherwise, untouched on a clip-idle frame.
#[derive(Resource, Debug, Default)]
pub struct LayerClips {
    /// Subtree member → how many leading rects of its real
    /// [`CalculatedClip`] come from above its layer root. Only members with
    /// at least one: the rest extract with their clip as it is.
    pub strip: EntityHashMap<usize>,
    /// Active root → the clip of its composite quad: its inherited
    /// [`CalculatedClip`] (top-level), or the part of it below the enclosing
    /// root (nested). Absent = unclipped.
    pub quads: EntityHashMap<QuadClip>,
    /// Nodes whose `strip` row appeared, changed or disappeared since
    /// [`mark_reclipped_members`] last drained this (see the module docs on
    /// retained extraction).
    pub changed: Vec<Entity>,
}

/// What clips a layer root's composite quad (physical px, screen space).
#[derive(Debug, Clone, PartialEq)]
pub enum QuadClip {
    /// Axis-aligned clippers only: exactly this rect. Empty = nothing shows.
    Rect(Rect),
    /// A rotated or skewed clipper among them: the clip rects themselves,
    /// for `bevy_ui_render`'s polygon clipper. Shared with the render world
    /// by reference count; allocated only when the clip changes.
    Rotated(Arc<CalculatedClip>),
}

impl QuadClip {
    /// `clip` without its first `skip` rects; `None` = unclipped.
    fn new(clip: &CalculatedClip, skip: usize) -> Option<Self> {
        let Some(rects) = clip.rects() else {
            return Some(Self::Rect(Rect::default()));
        };
        let rects = &rects[skip..];
        let upright = rects.iter().all(|c| {
            let m = c.world_to_clip_local.matrix2;
            m.x_axis.y == 0.0 && m.y_axis.x == 0.0
        });
        if upright {
            return tail_aabb(clip, skip).map(Self::Rect);
        }
        Some(Self::Rotated(Arc::new(CalculatedClip::Rects(
            rects.iter().copied().collect(),
        ))))
    }

    /// The clip's screen-space bounds: the clip itself for `Rect`, its
    /// bounding box for `Rotated`.
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Rect(rect) => *rect,
            // Never unclipped: the list holds the rotated clipper.
            Self::Rotated(clip) => clip_aabb(clip).unwrap_or(Rect::EMPTY),
        }
    }
}

/// The screen-space (physical px) bounds of an inherited [`CalculatedClip`]:
/// the intersection of every clip rect's world-space bounding box. `None` =
/// unclipped (no clip rects); a fully clipped node yields an empty rect. Exact
/// for clippers that only translate/scale; a rotated clipper contributes its
/// bounding box (the over-approximation errs on drawing, never on hiding).
pub fn clip_aabb(clip: &CalculatedClip) -> Option<Rect> {
    tail_aabb(clip, 0)
}

/// [`clip_aabb`] of `clip` without its first `skip` rects.
fn tail_aabb(clip: &CalculatedClip, skip: usize) -> Option<Rect> {
    /// Stands in for a `Visible` axis's infinite clip extent: transforming
    /// an infinite corner multiplies the matrix's zero terms by infinity
    /// (NaN), which `f32::min`/`max` then silently drop.
    const UNBOUNDED: f32 = 1e30;
    let Some(rects) = clip.rects() else {
        return Some(Rect::default());
    };
    rects[skip..].iter().fold(None, |bounds: Option<Rect>, c| {
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

/// How many leading rects of `clip` are `above`'s whole list — the part of a
/// node's clip that comes from above the layer root whose inherited clip is
/// `above`. 0 when the root is unclipped or either is fully clipped, and when
/// the lists differ (an `OverrideClip`/`FixedNode` reset in between).
fn inherited_len(clip: &CalculatedClip, above: Option<&CalculatedClip>) -> usize {
    match (clip.rects(), above.and_then(CalculatedClip::rects)) {
        (Some(rects), Some(above)) if rects.starts_with(above) => above.len(),
        _ => 0,
    }
}

/// Maintains [`LayerClips`]. Runs in `PostUpdate` after
/// [`super::sync_layer_geometry`] (this frame's [`LayerMembership`]) and after
/// `bevy_ui`'s `UiSystems::PostLayout` (this frame's [`CalculatedClip`]
/// values, the inserts and removals applied). Must NOT feed
/// [`super::resolve_layer_repaints`]: clip changes never dirty a capture —
/// that is the point.
///
/// A node's rows depend on its own clip list and its (enclosing) root's. A
/// membership rebuild re-derives every member's; otherwise only the nodes
/// whose own `CalculatedClip` changed or went away are looked at — a root's
/// list cannot change under a member that inherits it without the member's
/// changing too (it contains it), and a member that does not inherit it has
/// nothing to strip either way. Bevy writes the component only on a real
/// change, so a scroll (the clipper stays put) touches nothing here.
pub fn sync_layer_clips(
    real: Query<&CalculatedClip>,
    changed: Query<Entity, Changed<CalculatedClip>>,
    mut removed: RemovedComponents<CalculatedClip>,
    membership: Res<LayerMembership>,
    mut seen_rebuilds: Local<u64>,
    mut clips: ResMut<LayerClips>,
) {
    // `root` = the member's layer root (`node_to_layer`).
    let refresh = |entity: Entity, root: Entity, clips: &mut LayerClips| {
        let own = real.get(entity).ok();
        // As a member (a root is its own member: its paint is unclipped
        // inside its own capture).
        let strip = own.map_or(0, |clip| inherited_len(clip, real.get(root).ok()));
        let previous = match strip {
            0 => clips.strip.remove(&entity),
            _ => clips.strip.insert(entity, strip),
        };
        if previous.unwrap_or(0) != strip {
            clips.changed.push(entity);
        }
        // As an active root: its quad draws inside the enclosing capture, or
        // on screen.
        if entity == root
            && let Some(&outer) = membership.enclosing.get(&entity)
        {
            let quad = own.and_then(|clip| {
                let outer = outer.and_then(|outer| real.get(outer).ok());
                QuadClip::new(clip, inherited_len(clip, outer))
            });
            match quad {
                Some(quad) => clips.quads.insert(entity, quad),
                None => clips.quads.remove(&entity),
            };
        }
    };
    if *seen_rebuilds != membership.rebuilds {
        // Structural frame: membership is fresh, every row is suspect.
        *seen_rebuilds = membership.rebuilds;
        removed.clear();
        let clips = &mut *clips;
        // A node that left every layer extracts with its real clip again.
        let LayerClips { strip, changed, .. } = clips;
        strip.retain(|entity, _| {
            let member = membership.node_to_layer.contains_key(entity);
            if !member {
                changed.push(*entity);
            }
            member
        });
        clips.quads.clear();
        for (&member, &root) in &membership.node_to_layer {
            refresh(member, root, clips);
        }
        return;
    }
    if membership.node_to_layer.is_empty() {
        removed.clear();
        return; // No layers: nothing to derive, whatever clips changed.
    }
    let mut touched = changed
        .iter()
        .chain(removed.read())
        .filter_map(|entity| Some((entity, *membership.node_to_layer.get(&entity)?)))
        .peekable();
    if touched.peek().is_none() {
        return; // Clip-idle: last frame's maps stand (no `ResMut` deref).
    }
    let clips = &mut *clips;
    for (entity, root) in touched {
        refresh(entity, root, clips);
    }
}

/// Flag the `CalculatedClip` of every node whose `strip` row changed
/// ([`LayerClips::changed`]) so `bevy_ui_render`'s retained extraction
/// re-extracts it this frame, inside the swap window. Runs right after
/// [`sync_layer_clips`]. Entities without the component (unclipped, or
/// despawned) need nothing: there is no clip in their render data to refresh.
/// The flag reads as a clip change to [`sync_layer_clips`] next frame, which
/// finds the row unchanged and stops there.
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

#[cfg(test)]
pub(crate) mod tests;
