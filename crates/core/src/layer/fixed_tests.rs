//! A `FixedNode` is a layer boundary: it and its subtree are members of no
//! layer above it ([`LayerMembership`]). Hand-placed nodes on the clip
//! tests' harness (bevy's clipping, the geometry sync, the clip sync, the
//! repaint resolver).

use super::clip::tests::{clip_world, layer, node, promote};
use super::*;
use bevy::math::Affine2;
use bevy::ui::Overflow;

fn plain(world: &mut World, parent: Entity, center: Vec2) -> Entity {
    node(
        world,
        Some(parent),
        Vec2::splat(20.0),
        center,
        Overflow::visible(),
    )
}

fn layer_of(world: &World, e: Entity) -> Option<Entity> {
    world
        .resource::<LayerMembership>()
        .node_to_layer
        .get(&e)
        .copied()
}

/// The fixed node and everything under it stay out of the enclosing layer;
/// promoted, it claims its own subtree as a top-level layer, and so does a
/// root below an unpromoted fixed node.
#[test]
fn fixed_subtree_is_not_a_layer_member() {
    let (mut world, mut schedule) = clip_world();
    let outer = layer(
        &mut world,
        None,
        1,
        Vec2::splat(200.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    let member = plain(&mut world, outer, Vec2::splat(50.0));
    let fixed = plain(&mut world, outer, Vec2::splat(150.0));
    world.entity_mut(fixed).insert(FixedNode);
    let child = plain(&mut world, fixed, Vec2::splat(150.0));
    let nested = layer(
        &mut world,
        Some(child),
        2,
        Vec2::splat(10.0),
        Vec2::splat(150.0),
        Overflow::visible(),
    );
    schedule.run(&mut world);

    assert_eq!(layer_of(&world, outer), Some(outer));
    assert_eq!(layer_of(&world, member), Some(outer));
    assert_eq!(layer_of(&world, fixed), None);
    assert_eq!(layer_of(&world, child), None);
    assert_eq!(layer_of(&world, nested), Some(nested));
    let enclosing = |world: &World, e| world.resource::<LayerMembership>().enclosing[&e];
    assert_eq!(enclosing(&world, nested), None, "its quad draws on screen");

    promote(&mut world, fixed, 3);
    schedule.run(&mut world);
    assert_eq!(layer_of(&world, fixed), Some(fixed));
    assert_eq!(layer_of(&world, child), Some(fixed));
    assert_eq!(enclosing(&world, fixed), None, "top-level");
    assert_eq!(enclosing(&world, nested), Some(fixed));
}

/// Gaining or losing `FixedNode` is structural — nothing else need change —
/// and re-captures the layer the node left or joined. Once outside, the
/// node's own motion is not the layer's content.
#[test]
fn fixed_flip_rebuilds_membership() {
    let (mut world, mut schedule) = clip_world();
    let outer = layer(
        &mut world,
        None,
        1,
        Vec2::splat(200.0),
        Vec2::splat(100.0),
        Overflow::visible(),
    );
    let member = plain(&mut world, outer, Vec2::splat(50.0));
    schedule.run(&mut world);
    schedule.run(&mut world);
    let dirty = |world: &World| world.resource::<LayerRepaintState>().dirty.contains(&outer);
    let rebuilds = |world: &World| world.resource::<LayerMembership>().rebuilds;
    assert!(!dirty(&world), "settled");
    let settled = rebuilds(&world);

    world.entity_mut(member).insert(FixedNode);
    schedule.run(&mut world);
    assert_eq!(rebuilds(&world), settled + 1);
    assert_eq!(layer_of(&world, member), None);
    assert!(dirty(&world), "the capture held the node's pixels");

    // Outside the layer: moving it neither rebuilds nor repaints.
    *world.get_mut::<UiGlobalTransform>(member).unwrap() =
        UiGlobalTransform::from(Affine2::from_translation(Vec2::splat(80.0)));
    schedule.run(&mut world);
    assert_eq!(rebuilds(&world), settled + 1);
    assert!(!dirty(&world));

    world.entity_mut(member).remove::<FixedNode>();
    schedule.run(&mut world);
    assert_eq!(rebuilds(&world), settled + 2);
    assert_eq!(layer_of(&world, member), Some(outer));
    assert!(dirty(&world), "back in the capture");
}
