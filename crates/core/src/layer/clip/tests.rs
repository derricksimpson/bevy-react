//! The clip maps against Bevy's own clipping: `update_clipping_system`
//! computes every node's real `CalculatedClip` from hand-placed clipping
//! ancestors, [`sync_layer_clips`] derives the layers' view of it, and the
//! assertions read what extraction would — the member's clip inside the swap
//! window ([`interior_clip`]) and the root's quad clip.

use super::*;
use crate::layer::render::clip::{clip_quad, swap_in, swap_out};
use crate::layer::{
    LayerContentDirt, LayerGroupAlpha, LayerRepaintState, LayersRegistry, PromotedLayer,
    PromotionReasons, resolve_layer_repaints, sync_layer_geometry,
};
use crate::protocol::NodeId;
use bevy::math::Affine2;
use bevy::ui::update::update_clipping_system;
use bevy::ui::{ComputedNode, Overflow, OverrideClip, UiGlobalTransform};

/// World + schedule harness like `layer::tests::geometry_world`, with Bevy's
/// clipping ahead of it and [`sync_layer_clips`] between geometry and repaint
/// resolution — the production ordering.
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
            update_clipping_system,
            sync_layer_geometry,
            sync_layer_clips,
            resolve_layer_repaints,
        )
            .chain(),
    );
    (world, schedule)
}

/// A laid-out plain UI node under `parent`: `center` is the
/// `UiGlobalTransform` translation (node center), so the border box is
/// `center ± size/2`.
fn node(
    world: &mut World,
    parent: Option<Entity>,
    size: Vec2,
    center: Vec2,
    overflow: Overflow,
) -> Entity {
    let e = world
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
        .id();
    if let Some(parent) = parent {
        world.entity_mut(e).insert(ChildOf(parent));
    }
    e
}

/// A promoted layer root (adds the marker components
/// [`sync_layer_geometry`] requires on top of [`node`]).
fn layer(
    world: &mut World,
    parent: Option<Entity>,
    id: NodeId,
    size: Vec2,
    center: Vec2,
    overflow: Overflow,
) -> Entity {
    let e = node(world, parent, size, center, overflow);
    world.entity_mut(e).insert((
        crate::bridge::ReactNode(id),
        LayerGroupAlpha(1.0),
        PromotedLayer {
            reasons: PromotionReasons(PromotionReasons::FILTER),
        },
    ));
    e
}

/// The clip `e`'s items are extracted with — its `CalculatedClip` inside the
/// swap window. `None` = unclipped.
pub(crate) fn interior_clip(world: &mut World, e: Entity) -> Option<CalculatedClip> {
    let mut stash = Vec::new();
    swap_in(world, &mut stash);
    let clip = world.get::<CalculatedClip>(e).cloned();
    swap_out(world, &mut stash);
    clip.filter(|c| *c != CalculatedClip::default())
}

/// [`interior_clip`] as screen-space bounds.
fn interior(world: &mut World, e: Entity) -> Option<Rect> {
    interior_clip(world, e).as_ref().and_then(clip_aabb)
}

/// The node's real clip (what picking sees) as screen-space bounds.
fn real(world: &World, e: Entity) -> Option<Rect> {
    world.get::<CalculatedClip>(e).and_then(clip_aabb)
}

/// The bounds of a root's quad clip (`None` = unclipped).
fn quad(world: &World, e: Entity) -> Option<Rect> {
    world
        .resource::<LayerClips>()
        .quads
        .get(&e)
        .map(QuadClip::bounds)
}

/// Ancestor clipping (a scrollport above the root) is STRIPPED from every
/// member's interior clip — and becomes the top-level root's quad clip
/// instead. The real clips stay in place. A root without any clipping
/// ancestor gets an unclipped quad.
#[test]
fn ancestor_clip_stripped_and_moved_to_quad() {
    let (mut world, mut schedule) = clip_world();
    let port_rect = Rect::new(0.0, 0.0, 30.0, 30.0);
    let port = node(
        &mut world,
        None,
        Vec2::splat(30.0),
        Vec2::splat(15.0),
        Overflow::clip(),
    );
    let root = layer(
        &mut world,
        Some(port),
        1,
        Vec2::new(100.0, 60.0),
        Vec2::new(50.0, 30.0),
        Overflow::visible(),
    );
    let child = node(
        &mut world,
        Some(root),
        Vec2::new(40.0, 20.0),
        Vec2::new(50.0, 30.0),
        Overflow::visible(),
    );
    let unclipped = layer(
        &mut world,
        None,
        2,
        Vec2::new(50.0, 50.0),
        Vec2::new(300.0, 300.0),
        Overflow::visible(),
    );
    schedule.run(&mut world);

    assert_eq!(interior(&mut world, root), None);
    assert_eq!(interior(&mut world, child), None);
    assert_eq!(
        real(&world, root),
        Some(port_rect),
        "restored after the swap"
    );
    assert_eq!(real(&world, child), Some(port_rect));
    assert_eq!(quad(&world, root), Some(port_rect));
    assert_eq!(quad(&world, unclipped), None);
}

/// The root's own overflow clips its CHILDREN only (stock semantics):
/// the root's paint is unclipped in-capture, children clamp to the root's
/// `resolve_clip_rect`, and deeper interior clippers intersect — with a
/// scrollport above the root stripped from all of them.
#[test]
fn root_overflow_clips_children_only() {
    let (mut world, mut schedule) = clip_world();
    let port = node(
        &mut world,
        None,
        Vec2::splat(400.0),
        Vec2::splat(200.0),
        Overflow::clip(),
    );
    let root = layer(
        &mut world,
        Some(port),
        1,
        Vec2::splat(200.0),
        Vec2::splat(100.0),
        Overflow::clip(),
    );
    // Offset so the container's clip rect escapes the root's on the right.
    let container = node(
        &mut world,
        Some(root),
        Vec2::splat(100.0),
        Vec2::new(160.0, 100.0),
        Overflow::clip(),
    );
    let grandchild = node(
        &mut world,
        Some(container),
        Vec2::splat(40.0),
        Vec2::new(160.0, 100.0),
        Overflow::visible(),
    );
    schedule.run(&mut world);

    let root_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    let container_rect = Rect::new(110.0, 50.0, 210.0, 150.0);
    assert_eq!(interior(&mut world, root), None, "root paint unclipped");
    assert_eq!(interior(&mut world, container), Some(root_rect));
    assert_eq!(
        interior(&mut world, grandchild),
        Some(root_rect.intersect(container_rect)),
        "interior clippers intersect"
    );
    assert_eq!(quad(&world, root), Some(Rect::new(0.0, 0.0, 400.0, 400.0)));
}

/// Nested layers: a member loses everything above its NEAREST root, and a
/// nested root's quad carries only the clippers below its enclosing root —
/// outer clips must not leak in.
#[test]
fn nested_roots_get_quad_clips_and_restart_interior() {
    let (mut world, mut schedule) = clip_world();
    let port = node(
        &mut world,
        None,
        Vec2::splat(400.0),
        Vec2::splat(200.0),
        Overflow::clip(),
    );
    let outer = layer(
        &mut world,
        Some(port),
        1,
        Vec2::splat(300.0),
        Vec2::splat(150.0),
        Overflow::visible(),
    );
    let clipper = node(
        &mut world,
        Some(outer),
        Vec2::splat(100.0),
        Vec2::splat(100.0),
        Overflow::clip(),
    );
    let inner = layer(
        &mut world,
        Some(clipper),
        2,
        Vec2::splat(80.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    let inner_child = node(
        &mut world,
        Some(inner),
        Vec2::splat(40.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    // Deeper: a clipper partially OUTSIDE the outer clipper's rect, then
    // a third root — its quad clip must be the inner clipper alone
    // (restart proof: no intersection with the outer clipper's rect).
    let clipper2 = node(
        &mut world,
        Some(inner),
        Vec2::splat(60.0),
        Vec2::new(140.0, 100.0),
        Overflow::clip(),
    );
    let innermost = layer(
        &mut world,
        Some(clipper2),
        3,
        Vec2::splat(40.0),
        Vec2::new(140.0, 100.0),
        Overflow::visible(),
    );
    schedule.run(&mut world);

    let clipper_rect = Rect::new(50.0, 50.0, 150.0, 150.0);
    let clipper2_rect = Rect::new(110.0, 70.0, 170.0, 130.0);
    assert_eq!(quad(&world, outer), Some(Rect::new(0.0, 0.0, 400.0, 400.0)));
    assert_eq!(interior(&mut world, clipper), None, "outer member");
    assert_eq!(quad(&world, inner), Some(clipper_rect));
    assert_eq!(interior(&mut world, inner), None, "interior restarts");
    assert_eq!(interior(&mut world, inner_child), None);
    assert_eq!(interior(&mut world, clipper2), None);
    assert_eq!(
        quad(&world, innermost),
        Some(clipper2_rect),
        "the inner clipper only — the outer one must not leak through the restart"
    );
    assert_eq!(interior(&mut world, innermost), None);
}

/// `OverrideClip` resets the inherited clip (a clipper below it starts a new
/// list, kept whole); `Display::None` fully clips its subtree (stock
/// `update_clipping` order).
#[test]
fn override_clip_and_display_none() {
    let (mut world, mut schedule) = clip_world();
    let port = node(
        &mut world,
        None,
        Vec2::splat(400.0),
        Vec2::splat(200.0),
        Overflow::clip(),
    );
    let root = layer(
        &mut world,
        Some(port),
        1,
        Vec2::splat(200.0),
        Vec2::splat(100.0),
        Overflow::clip(),
    );
    let root_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    let leaf = |world: &mut World, parent: Entity, px: f32| {
        node(
            world,
            Some(parent),
            Vec2::splat(px),
            Vec2::splat(100.0),
            Overflow::visible(),
        )
    };

    let escapee = leaf(&mut world, root, 40.0);
    world.entity_mut(escapee).insert(OverrideClip);
    let escapee_child = leaf(&mut world, escapee, 20.0);
    let escapee_clipper = node(
        &mut world,
        Some(escapee),
        Vec2::splat(30.0),
        Vec2::splat(100.0),
        Overflow::clip(),
    );
    let escapee_leaf = leaf(&mut world, escapee_clipper, 20.0);

    let hidden = leaf(&mut world, root, 40.0);
    world.entity_mut(hidden).get_mut::<Node>().unwrap().display = bevy::ui::Display::None;
    let hidden_child = leaf(&mut world, hidden, 20.0);

    let plain = leaf(&mut world, root, 40.0);
    schedule.run(&mut world);

    assert_eq!(interior(&mut world, escapee), None);
    assert_eq!(interior(&mut world, escapee_child), None);
    assert_eq!(
        interior(&mut world, escapee_leaf),
        Some(Rect::new(85.0, 85.0, 115.0, 115.0)),
        "the list restarted below the escapee stays whole"
    );
    assert_eq!(interior(&mut world, hidden), Some(Rect::default()));
    assert_eq!(interior(&mut world, hidden_child), Some(Rect::default()));
    assert_eq!(interior(&mut world, plain), Some(root_rect));
}

/// THE regression the whole change exists for: scrolling (uniform
/// translation of the subtree under a fixed scrollport) must keep the
/// capture cache clean — clips never feed the repaint resolver — while
/// the quad clip stays live; and a scrollport resize alone must not dirty
/// either.
#[test]
fn scroll_holds_cache_and_updates_quad_clip() {
    let (mut world, mut schedule) = clip_world();
    let clip = Rect::new(0.0, 0.0, 200.0, 100.0);
    let port = node(
        &mut world,
        None,
        Vec2::new(200.0, 100.0),
        Vec2::new(100.0, 50.0),
        Overflow::clip(),
    );
    // Content sits below the 100-tall scrollport (fully clipped away).
    let root = layer(
        &mut world,
        Some(port),
        1,
        Vec2::new(100.0, 60.0),
        Vec2::new(50.0, 130.0),
        Overflow::visible(),
    );
    let child = node(
        &mut world,
        Some(root),
        Vec2::new(40.0, 20.0),
        Vec2::new(50.0, 130.0),
        Overflow::visible(),
    );
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

    // The scrollport itself grows: quad clip follows, still no dirt.
    let grown = Rect::new(0.0, 0.0, 200.0, 150.0);
    world.get_mut::<ComputedNode>(port).unwrap().size = grown.size();
    *world.get_mut::<UiGlobalTransform>(port).unwrap() =
        UiGlobalTransform::from(Affine2::from_translation(grown.center()));
    schedule.run(&mut world);
    assert!(
        world.resource::<LayerRepaintState>().dirty.is_empty(),
        "a clip change must not re-capture"
    );
    assert_eq!(quad(&world, root), Some(grown));
    assert_eq!(interior(&mut world, child), None);
}

/// Whether `e`'s `CalculatedClip` was written or flagged since the last
/// `World::clear_trackers` — what retained extraction re-extracts on.
fn flagged(world: &World, e: Entity) -> bool {
    world
        .entity(e)
        .get_ref::<CalculatedClip>()
        .unwrap()
        .is_changed()
}

/// Retained extraction keeps the clip of a node's last extraction in its
/// render data, so a membership transition must flag the member's
/// `CalculatedClip` (re-extracted inside the swap window) — joining a
/// layer and leaving it — while the frame after flags nothing (the flag
/// itself reads as a clip change next frame; it must not feed back).
#[test]
fn membership_transitions_flag_clips_for_reextraction() {
    let (mut world, mut schedule) = clip_world();
    schedule.add_systems(mark_reclipped_members.after(sync_layer_clips));
    let mut frame = |world: &mut World| {
        world.clear_trackers();
        schedule.run(world);
    };
    let clip = Rect::new(0.0, 0.0, 30.0, 30.0);
    let port = node(
        &mut world,
        None,
        Vec2::splat(30.0),
        Vec2::splat(15.0),
        Overflow::clip(),
    );
    let root = layer(
        &mut world,
        Some(port),
        1,
        Vec2::new(100.0, 60.0),
        Vec2::new(50.0, 30.0),
        Overflow::visible(),
    );
    let child = node(
        &mut world,
        Some(root),
        Vec2::new(40.0, 20.0),
        Vec2::new(50.0, 30.0),
        Overflow::visible(),
    );
    frame(&mut world);
    assert!(world.resource::<LayerClips>().changed.is_empty(), "drained");

    // Idle: nothing joins or leaves, nothing is flagged.
    frame(&mut world);
    frame(&mut world);
    assert!(!flagged(&world, root) && !flagged(&world, child), "idle");

    // Demotion: the members' render data holds the interior clip.
    world.entity_mut(root).remove::<PromotedLayer>();
    frame(&mut world);
    assert!(flagged(&world, root) && flagged(&world, child), "demoted");
    assert_eq!(interior(&mut world, child), Some(clip), "the real clip");
    frame(&mut world);
    assert!(!flagged(&world, root) && !flagged(&world, child), "once");

    // Promotion: the members' render data holds the real ancestor clip.
    world.entity_mut(root).insert(PromotedLayer {
        reasons: PromotionReasons(PromotionReasons::FILTER),
    });
    frame(&mut world);
    assert!(flagged(&world, root) && flagged(&world, child), "promoted");
    assert_eq!(interior(&mut world, child), None);
    frame(&mut world);
    assert!(!flagged(&world, root) && !flagged(&world, child), "once");
}

/// A layer with no clipping ancestor has nothing to strip: its members
/// extract with their real clips in and out of the layer, so nothing is
/// swapped and a promotion flip re-extracts none of them — an interior
/// clipper's children included.
#[test]
fn a_root_without_clipping_ancestors_swaps_and_flags_nothing() {
    let (mut world, mut schedule) = clip_world();
    schedule.add_systems(mark_reclipped_members.after(sync_layer_clips));
    let mut frame = |world: &mut World| {
        world.clear_trackers();
        schedule.run(world);
    };
    let root = layer(
        &mut world,
        None,
        1,
        Vec2::splat(200.0),
        Vec2::splat(100.0),
        Overflow::clip(),
    );
    let child = node(
        &mut world,
        Some(root),
        Vec2::splat(40.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    let root_rect = Rect::new(0.0, 0.0, 200.0, 200.0);
    frame(&mut world);
    for promoted in [false, true, false] {
        if promoted {
            world.entity_mut(root).insert(PromotedLayer {
                reasons: PromotionReasons(PromotionReasons::FILTER),
            });
        } else {
            world.entity_mut(root).remove::<PromotedLayer>();
        }
        frame(&mut world);
        assert!(!flagged(&world, child), "promoted: {promoted}");
        assert!(world.resource::<LayerClips>().strip.is_empty());
        assert_eq!(interior(&mut world, child), Some(root_rect));
    }
}

/// A scroll container clips one axis only: bevy_ui gives its `Visible`
/// axis an infinite extent, which must stay unbounded in the screen
/// bounds (transforming it naively yields NaN, i.e. an empty rect that
/// clips a nested layer's quad away entirely) — and must still be
/// recognized as the part above the nested root.
#[test]
fn single_axis_clip_bounds_stay_unbounded_on_the_open_axis() {
    let (mut world, mut schedule) = clip_world();
    let outer = layer(
        &mut world,
        None,
        1,
        Vec2::splat(300.0),
        Vec2::splat(150.0),
        Overflow::visible(),
    );
    let scroller = node(
        &mut world,
        Some(outer),
        Vec2::new(200.0, 100.0),
        Vec2::new(150.0, 100.0),
        Overflow::scroll_y(),
    );
    let inner = layer(
        &mut world,
        Some(scroller),
        2,
        Vec2::splat(40.0),
        Vec2::new(150.0, 100.0),
        Overflow::visible(),
    );
    let inner_child = node(
        &mut world,
        Some(inner),
        Vec2::splat(20.0),
        Vec2::new(150.0, 100.0),
        Overflow::visible(),
    );
    schedule.run(&mut world);

    let clip = quad(&world, inner).expect("the scroller clips the nested quad");
    assert_eq!((clip.min.y, clip.max.y), (50.0, 150.0), "the clipped axis");
    assert!(
        clip.min.x < -1e20 && clip.max.x > 1e20,
        "the open axis stays unbounded, got {clip:?}"
    );
    assert!(
        clip_quad(
            Vec2::new(130.0, 80.0),
            bevy::math::UVec2::splat(40),
            0.0,
            Some(clip)
        )
        .is_some(),
        "the nested quad still draws"
    );
    assert_eq!(interior(&mut world, inner), None);
    assert_eq!(interior(&mut world, inner_child), None);
    assert_eq!(quad(&world, outer), None);
}

/// An interior clipper keeps Bevy's own clip rect — in the clipper's local
/// space, rotation included — while the scrollport above the root goes.
#[test]
fn interior_keeps_a_rotated_clippers_affine_rect() {
    let (mut world, mut schedule) = clip_world();
    let port = node(
        &mut world,
        None,
        Vec2::splat(400.0),
        Vec2::splat(200.0),
        Overflow::clip(),
    );
    let root = layer(
        &mut world,
        Some(port),
        1,
        Vec2::splat(200.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    let clipper = node(
        &mut world,
        Some(root),
        Vec2::splat(100.0),
        Vec2::splat(100.0),
        Overflow::clip(),
    );
    let turned = Affine2::from_angle_translation(30f32.to_radians(), Vec2::splat(100.0));
    *world.get_mut::<UiGlobalTransform>(clipper).unwrap() = UiGlobalTransform::from(turned);
    let member = node(
        &mut world,
        Some(clipper),
        Vec2::splat(40.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    schedule.run(&mut world);

    let clip = interior_clip(&mut world, member).expect("the clipper's rect");
    let [only] = clip.rects().expect("not fully clipped") else {
        panic!("one rect, got {clip:?}");
    };
    assert_eq!(
        only.rect,
        Rect::from_center_size(Vec2::ZERO, Vec2::splat(100.0))
    );
    assert_eq!(only.world_to_clip_local, turned.inverse());
    assert_eq!(quad(&world, root), Some(Rect::new(0.0, 0.0, 400.0, 400.0)));
}

/// The quad clip stays one screen rect while the clipping ancestors only
/// translate and scale (a layout transition's FLIP scale), turns into the
/// clip rects themselves once one rotates, and is the empty rect under a
/// fully clipped root.
#[test]
fn quad_clip_is_a_rect_unless_a_clipper_rotates() {
    let (mut world, mut schedule) = clip_world();
    let port = node(
        &mut world,
        None,
        Vec2::splat(100.0),
        Vec2::splat(100.0),
        Overflow::clip(),
    );
    let root = layer(
        &mut world,
        Some(port),
        1,
        Vec2::splat(40.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    let mut quad_clip = |world: &mut World, port_transform: Affine2| {
        *world.get_mut::<UiGlobalTransform>(port).unwrap() =
            UiGlobalTransform::from(port_transform);
        schedule.run(world);
        world.resource::<LayerClips>().quads.get(&root).cloned()
    };

    let scaled =
        Affine2::from_scale_angle_translation(Vec2::new(2.0, 0.5), 0.0, Vec2::splat(100.0));
    assert_eq!(
        quad_clip(&mut world, scaled),
        Some(QuadClip::Rect(Rect::new(0.0, 75.0, 200.0, 125.0)))
    );

    let turned = Affine2::from_angle_translation(45f32.to_radians(), Vec2::splat(100.0));
    let Some(QuadClip::Rotated(clip)) = quad_clip(&mut world, turned) else {
        panic!("a rotated clipper keeps its rects");
    };
    let mut expected = CalculatedClip::default();
    expected.push_rect(
        Rect::from_center_size(Vec2::ZERO, Vec2::splat(100.0)),
        turned.inverse(),
    );
    assert_eq!(*clip, expected);
    let bounds = quad(&world, root).unwrap();
    let reach = 50.0 * 2f32.sqrt();
    assert!(
        bounds.min.abs_diff_eq(Vec2::splat(100.0 - reach), 1e-3),
        "{bounds:?}"
    );
    assert!(
        bounds.max.abs_diff_eq(Vec2::splat(100.0 + reach), 1e-3),
        "{bounds:?}"
    );

    world.get_mut::<Node>(port).unwrap().display = bevy::ui::Display::None;
    assert_eq!(
        quad_clip(&mut world, scaled),
        Some(QuadClip::Rect(Rect::default()))
    );
}
