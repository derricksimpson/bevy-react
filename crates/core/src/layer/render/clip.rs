//! Render-world half of clip-independent layer capture: the extract-window
//! clip swap, and the composite quad's rect clamp.
//!
//! **The swap.** Stock `bevy_ui_render` extraction copies each node's
//! [`CalculatedClip`] onto its extracted items, and the prepare stage bakes
//! that clip into vertices — including inside our capture passes. To keep
//! ancestor clips out of captures without forking any extractor (several,
//! like the gradient one, live in private modules), the members' clips lose
//! the rects that come from above their layer root (the counts in
//! `crate::layer::clip::LayerClips::strip`) for **exactly the duration of
//! `ExtractSchedule`**: [`swap_interior_clips_in`] runs before every stock
//! UI extraction set and [`swap_interior_clips_out`] restores the originals
//! after them. The main world is exclusively borrowed for the whole window
//! (that is what `ResMut<MainWorld>` means), so no main-world system —
//! picking, focus, `bevy_ui` itself — can ever observe a swapped value.
//!
//! Swap mechanics: only members with something to strip are touched (a layer
//! with no clipping ancestor costs nothing), values are overwritten in place
//! (never inserted/removed, so there is no archetype churn), "unclipped" is
//! an empty clip-rect list (`CalculatedClip::default()`, bevy_ui's own "no
//! inherited clip" value), and both directions bypass change detection (the
//! net effect within a frame is identity; `update_clipping`'s own `!=` guard
//! must not see churn).
//!
//! **The quad clamp.** With captures unclipped, the composite quad applies
//! the ancestor clipping instead: [`clip_quad`] clamps the quad's corners to
//! the layer's quad clip and shifts UVs proportionally. A fully clipped-away
//! layer yields `None` — its quad simply isn't batched. Under a rotated
//! clipping ancestor a rect cannot say it: [`clip_quad_rotated`] cuts the
//! quad to the clip's polygon with `bevy_ui_render`'s own clipper (what stock
//! prepare runs on every item), so the layer ends where its unpromoted
//! content would.

use bevy::math::{Rect, UVec2, Vec2};
use bevy::prelude::*;
use bevy::render::MainWorld;
use bevy::ui::CalculatedClip;

use crate::layer::clip::LayerClips;

/// The originals stashed by [`swap_interior_clips_in`], restored by
/// [`swap_interior_clips_out`]. Render-world resource; drained every frame.
#[derive(Resource, Default)]
pub struct SwappedClips(pub Vec<(Entity, CalculatedClip)>);

/// `ExtractSchedule`, before all stock UI extraction sets: drop, from each
/// promoted-subtree member's [`CalculatedClip`], the rects inherited from
/// above its layer root.
pub fn swap_interior_clips_in(mut main_world: ResMut<MainWorld>, mut stash: ResMut<SwappedClips>) {
    swap_in(&mut main_world, &mut stash.0);
}

/// `ExtractSchedule`, after all stock UI extraction sets: restore the
/// originals so the main world resumes with true inherited clips in place.
pub fn swap_interior_clips_out(mut main_world: ResMut<MainWorld>, mut stash: ResMut<SwappedClips>) {
    swap_out(&mut main_world, &mut stash.0);
}

/// Core of [`swap_interior_clips_in`], factored on `&mut World` for tests.
/// Every `strip` row is a real swap; the original moves into `stash` (no
/// clone), and what replaces it stays inline up to two remaining rects.
pub fn swap_in(world: &mut World, stash: &mut Vec<(Entity, CalculatedClip)>) {
    stash.clear();
    // Also absent in a main app without the plugin's resources.
    if world
        .get_resource::<LayerClips>()
        .is_none_or(|clips| clips.strip.is_empty())
    {
        return;
    }
    world.resource_scope(|world, clips: Mut<LayerClips>| {
        for (&entity, &strip) in &clips.strip {
            // A despawned entity is a stale row until the next rebuild.
            let Ok(mut e) = world.get_entity_mut(entity) else {
                continue;
            };
            let Some(mut clip) = e.get_mut::<CalculatedClip>() else {
                continue;
            };
            let Some(rects) = clip.rects() else {
                continue;
            };
            let mut interior = CalculatedClip::default();
            for rect in &rects[strip.min(rects.len())..] {
                interior.push_rect(rect.rect, rect.world_to_clip_local);
            }
            let original = std::mem::replace(clip.bypass_change_detection(), interior);
            stash.push((entity, original));
        }
    });
}

/// Core of [`swap_interior_clips_out`]: restore every stashed original.
pub fn swap_out(world: &mut World, stash: &mut Vec<(Entity, CalculatedClip)>) {
    for (entity, original) in stash.drain(..) {
        if let Ok(mut e) = world.get_entity_mut(entity)
            && let Some(mut clip) = e.get_mut::<CalculatedClip>()
        {
            *clip.bypass_change_detection() = original;
        }
    }
}

/// A composite quad clamped to its clip: screen-space corners plus the
/// matching capture-texture UV window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClippedQuad {
    pub pos_min: Vec2,
    pub pos_max: Vec2,
    pub uv_min: Vec2,
    pub uv_max: Vec2,
}

/// A layer's composite quad over its capture rect (`min`, `size`): the rect
/// shrunk by `inset` on every side (negative grows it), clamped to `clip`,
/// with UVs relative to the WHOLE capture rect — so the visible part of the
/// capture stays put on screen. The content quad insets nothing, the
/// backdrop quad its blur outset (frost never paints in the outset ring),
/// a transformed quad `-EDGE_AA_INFLATE_PX` (UVs run past `[0, 1]`, the
/// shader feathers the true edge). `None` = an empty or degenerate result
/// (draw nothing).
pub fn clip_quad(min: Vec2, size: UVec2, inset: f32, clip: Option<Rect>) -> Option<ClippedQuad> {
    let size = size.as_vec2();
    let (box_min, box_max) = (min + inset, min + size - inset);
    let (pos_min, pos_max) = match clip {
        None => (box_min, box_max),
        Some(c) => (box_min.max(c.min), box_max.min(c.max)),
    };
    if pos_min.x >= pos_max.x || pos_min.y >= pos_max.y {
        return None;
    }
    // `max(ONE)`: a zero-size rect grown by the edge feather still maps
    // finite UVs.
    let uv_size = size.max(Vec2::ONE);
    Some(ClippedQuad {
        pos_min,
        pos_max,
        uv_min: (pos_min - min) / uv_size,
        uv_max: (pos_max - min) / uv_size,
    })
}

/// The layer's quad (as [`clip_quad`] lays it out, unclipped) cut to `clip`
/// and staged as a triangle list through `push(position, uv)`. Returns the
/// vertex count — 0 when nothing shows. For clips a screen rect cannot
/// express (a rotated clipper); `clip_quad` stays the path for the rest.
pub fn clip_quad_rotated(
    min: Vec2,
    size: UVec2,
    inset: f32,
    clip: &CalculatedClip,
    mut push: impl FnMut(Vec2, Vec2),
) -> u32 {
    let Some(q) = clip_quad(min, size, inset, None) else {
        return 0;
    };
    let corners = [
        q.pos_min,
        Vec2::new(q.pos_max.x, q.pos_min.y),
        q.pos_max,
        Vec2::new(q.pos_min.x, q.pos_max.y),
    ]
    .map(|corner| (corner, ()));
    // A convex polygon in boundary order (empty when clipped away), fanned
    // from its first vertex — as stock prepare does.
    let polygon = bevy::ui_render::clipping::clip_polygon(Some(clip), &corners, |_, _, _| ());
    // The capture maps affinely onto its rect, so a vertex's UV follows from
    // its position (as in `clip_quad`) — nothing to interpolate.
    let uv_size = size.as_vec2().max(Vec2::ONE);
    let triangles = polygon.len().saturating_sub(2);
    for i in 1..=triangles {
        for (position, ()) in [polygon[0], polygon[i], polygon[i + 1]] {
            push(position, (position - min) / uv_size);
        }
    }
    3 * triangles as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `clip_quad` truth table: full quad without a clip, proportional UV
    /// shifts on clamped sides, identity for a containing clip, `None` for
    /// empty/degenerate intersections.
    #[test]
    fn clip_quad_table() {
        let min = Vec2::new(10.0, 20.0);
        let size = UVec2::new(100, 50);

        // No clip: full quad, full UV window.
        let full = clip_quad(min, size, 0.0, None).expect("unclipped quad");
        assert_eq!(full.pos_min, min);
        assert_eq!(full.pos_max, Vec2::new(110.0, 70.0));
        assert_eq!(full.uv_min, Vec2::ZERO);
        assert_eq!(full.uv_max, Vec2::ONE);

        // Left half clipped away: UV window starts at 0.5 horizontally.
        let q = clip_quad(min, size, 0.0, Some(Rect::new(60.0, 0.0, 300.0, 300.0)))
            .expect("partial overlap");
        assert_eq!(q.pos_min, Vec2::new(60.0, 20.0));
        assert_eq!(q.pos_max, Vec2::new(110.0, 70.0));
        assert_eq!(q.uv_min, Vec2::new(0.5, 0.0));
        assert_eq!(q.uv_max, Vec2::ONE);

        // Bottom 40% clipped away: UV max shrinks to 0.6 vertically.
        let q = clip_quad(min, size, 0.0, Some(Rect::new(0.0, 0.0, 300.0, 50.0)))
            .expect("partial overlap");
        assert_eq!(q.pos_min, min);
        assert_eq!(q.pos_max, Vec2::new(110.0, 50.0));
        assert_eq!(q.uv_min, Vec2::ZERO);
        assert_eq!(q.uv_max, Vec2::new(1.0, 0.6));

        // Containing clip: identity.
        let q = clip_quad(min, size, 0.0, Some(Rect::new(0.0, 0.0, 500.0, 500.0)))
            .expect("containing clip");
        assert_eq!(q, full);

        // Disjoint clip: nothing to draw.
        assert_eq!(
            clip_quad(min, size, 0.0, Some(Rect::new(200.0, 0.0, 300.0, 300.0))),
            None
        );
        // Degenerate touch (shared edge): still nothing.
        assert_eq!(
            clip_quad(min, size, 0.0, Some(Rect::new(110.0, 0.0, 300.0, 300.0))),
            None
        );
        // Empty clip rect (Display::None subtree): nothing.
        assert_eq!(clip_quad(min, size, 0.0, Some(Rect::default())), None);
    }

    /// A clip rotated 45° about the quad's center cuts its four corners off:
    /// an octagon, fanned into triangles that cover exactly the visible part,
    /// every vertex inside both the quad and the clip, UVs following
    /// positions. A clip containing the quad leaves the plain two triangles;
    /// a disjoint one leaves nothing.
    #[test]
    fn rotated_clip_cuts_the_quad_to_a_polygon() {
        use bevy::math::Affine2;
        let min = Vec2::new(10.0, 20.0);
        let size = UVec2::splat(100);
        let center = min + 50.0;
        let diamond = |side: f32, center: Vec2| {
            let mut clip = CalculatedClip::default();
            clip.push_rect(
                Rect::from_center_size(Vec2::ZERO, Vec2::splat(side)),
                Affine2::from_angle_translation(45f32.to_radians(), center).inverse(),
            );
            clip
        };
        let cut = |clip: &CalculatedClip| {
            let mut vertices = Vec::new();
            let count = clip_quad_rotated(min, size, 0.0, clip, |p, uv| vertices.push((p, uv)));
            assert_eq!(count as usize, vertices.len());
            vertices
        };

        let clip = diamond(100.0, center);
        let vertices = cut(&clip);
        assert_eq!(vertices.len(), 18, "an octagon: six triangles");
        let mut area = 0.0;
        for triangle in vertices.chunks(3) {
            let [(a, _), (b, _), (c, _)] = triangle else {
                unreachable!()
            };
            area += (*b - *a).perp_dot(*c - *a).abs() / 2.0;
        }
        // The square minus its four corners beyond the diamond's edges.
        let corner = 100.0 - 50.0 * 2f32.sqrt();
        assert!(
            (area - (10_000.0 - 2.0 * corner * corner)).abs() < 0.1,
            "{area}"
        );
        let rect = clip.rects().unwrap()[0];
        for &(p, uv) in &vertices {
            let local = rect.world_to_clip_local.transform_point2(p);
            assert!(
                rect.rect.inflate(1e-3).contains(local),
                "{p} outside the clip"
            );
            assert!(
                Rect::from_corners(min, min + 100.0)
                    .inflate(1e-3)
                    .contains(p)
            );
            assert!(uv.abs_diff_eq((p - min) / 100.0, 1e-6));
        }

        let full = clip_quad(min, size, 0.0, None).unwrap();
        let corners = [
            (full.pos_min, Vec2::ZERO),
            (Vec2::new(full.pos_max.x, full.pos_min.y), Vec2::X),
            (full.pos_max, Vec2::ONE),
            (full.pos_min, Vec2::ZERO),
            (full.pos_max, Vec2::ONE),
            (Vec2::new(full.pos_min.x, full.pos_max.y), Vec2::Y),
        ];
        assert_eq!(cut(&diamond(1000.0, center)), corners, "the plain quad");
        assert_eq!(cut(&diamond(100.0, center + 500.0)), [], "disjoint");
        assert_eq!(cut(&CalculatedClip::FullyClipped), []);
    }

    /// The swap drops each member's leading `strip` rects (leaving an empty
    /// list for "unclipped inside the capture"), stashes the originals,
    /// leaves everything without a row alone, and restores exactly.
    #[test]
    fn swap_roundtrip() {
        use crate::layer::clip::screen_clip;
        let mut world = World::new();
        let outer = Rect::new(0.0, 0.0, 100.0, 100.0);
        let inner = Rect::new(10.0, 10.0, 50.0, 50.0);
        let one = screen_clip(outer);
        let mut two = one.clone();
        two.push_rect(inner, bevy::math::Affine2::IDENTITY);

        // One rect from above the root, one from inside: the inner one stays.
        let a = world.spawn(two.clone()).id();
        // Everything from above the root: unclipped inside the capture.
        let b = world.spawn(one.clone()).id();
        // No row: untouched (a non-member, or nothing to strip).
        let outside = world.spawn(two.clone()).id();
        // Rows gone stale: a removed component, a despawned entity, a clip
        // that turned fully clipped or got shorter than the count.
        let bare = world.spawn_empty().id();
        let dead = world.spawn_empty().id();
        world.despawn(dead);
        let hidden = world.spawn(CalculatedClip::FullyClipped).id();
        let shorter = world.spawn(one.clone()).id();

        let mut clips = LayerClips::default();
        clips.strip.extend([
            (a, 1),
            (b, 1),
            (bare, 1),
            (dead, 1),
            (hidden, 1),
            (shorter, 2),
        ]);
        world.insert_resource(clips);

        let mut stash = Vec::new();
        swap_in(&mut world, &mut stash);
        assert_eq!(world.get::<CalculatedClip>(a).unwrap(), &screen_clip(inner));
        assert_eq!(
            world.get::<CalculatedClip>(b).unwrap(),
            &CalculatedClip::default()
        );
        assert_eq!(world.get::<CalculatedClip>(outside).unwrap(), &two);
        assert!(world.get::<CalculatedClip>(bare).is_none());
        assert_eq!(
            world.get::<CalculatedClip>(hidden).unwrap(),
            &CalculatedClip::FullyClipped
        );
        assert_eq!(
            world.get::<CalculatedClip>(shorter).unwrap(),
            &CalculatedClip::default()
        );
        assert_eq!(stash.len(), 3, "only really-swapped members are stashed");

        swap_out(&mut world, &mut stash);
        assert_eq!(world.get::<CalculatedClip>(a).unwrap(), &two);
        assert_eq!(world.get::<CalculatedClip>(b).unwrap(), &one);
        assert_eq!(world.get::<CalculatedClip>(shorter).unwrap(), &one);
        assert!(stash.is_empty(), "stash drains on restore");
    }

    /// A second frame's swap starts from a clean stash even if the previous
    /// restore was somehow skipped (defensive: swap_in clears).
    #[test]
    fn swap_in_clears_previous_stash() {
        let mut world = World::new();
        world.insert_resource(LayerClips::default());
        let mut stash = vec![(Entity::PLACEHOLDER, CalculatedClip::default())];
        swap_in(&mut world, &mut stash);
        assert!(stash.is_empty());
    }
}
