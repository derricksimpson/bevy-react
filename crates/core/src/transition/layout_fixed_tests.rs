//! `transition: { layout }` around a `FixedNode`: bevy lays it out as its own
//! root, against the viewport — its rect is not in its parent's space, and
//! nothing above it reaches it. Real layout (the harness of
//! [`super::layout_tests`], a 1000×100 target).
use super::layout_tests::{Written, global, layout_app, layout_spec, root_row, square, step, x_of};
use super::shared::{SharedRect, SharedSeed};
use super::tests::timing;
use super::*;
use crate::animations::Easing;
use bevy::ui::{FixedNode, LayoutConfig, UiTransform};

/// A 20×20 absolute node `left` px from its containing block's left edge.
fn pinned(left: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(0.0),
        ..square(20.0)
    }
}

/// Row `[A][B]` with a fixed node under B; remove A. B eases over — and the
/// fixed node, which bevy never moved, is not carried along with B's
/// subtree.
#[test]
fn fixed_descendant_does_not_ride_an_ancestors_flip() {
    let mut app = layout_app();
    let root = app.world_mut().spawn(root_row()).id();
    let a = app.world_mut().spawn((square(100.0), ChildOf(root))).id();
    let b = app
        .world_mut()
        .spawn((
            square(100.0),
            ChildOf(root),
            layout_spec(1.0),
            TransitionState::default(),
        ))
        .id();
    let fixed = app
        .world_mut()
        .spawn((pinned(40.0), FixedNode, ChildOf(b)))
        .id();
    step(&mut app, 0.016);
    let pristine = global(&app, fixed);
    assert_eq!(
        pristine.translation,
        Vec2::new(50.0, 10.0),
        "the viewport's"
    );

    app.world_mut().entity_mut(a).despawn();
    step(&mut app, 0.0);
    assert_eq!(
        x_of(&app, b),
        150.0,
        "B: the change frame shows the old rect"
    );
    assert_eq!(global(&app, fixed), pristine);
    step(&mut app, 0.5);
    assert!((x_of(&app, b) - 100.0).abs() < 1e-3, "B: halfway");
    assert_eq!(global(&app, fixed), pristine);
    assert!(
        !app.world().resource::<Written>().0.contains(&fixed),
        "no global-transform write on the fixed node"
    );
}

/// A fixed node's own ease runs in the viewport's space: its parent's scale
/// reaches neither the node nor its delta.
#[test]
fn fixed_node_eases_in_viewport_space() {
    let mut app = layout_app();
    let root = app.world_mut().spawn(root_row()).id();
    let parent = app
        .world_mut()
        .spawn((
            square(100.0),
            UiTransform::from_scale(Vec2::splat(2.0)),
            ChildOf(root),
        ))
        .id();
    let fixed = app
        .world_mut()
        .spawn((
            pinned(0.0),
            FixedNode,
            ChildOf(parent),
            layout_spec(1.0),
            TransitionState::default(),
        ))
        .id();
    step(&mut app, 0.016);
    assert_eq!(x_of(&app, fixed), 10.0);

    app.world_mut().get_mut::<Node>(fixed).unwrap().left = Val::Px(100.0);
    step(&mut app, 0.0);
    assert_eq!(x_of(&app, fixed), 10.0, "change frame shows the old rect");
    step(&mut app, 0.5);
    assert!(
        (x_of(&app, fixed) - 60.0).abs() < 1e-3,
        "halfway, unscaled: {}",
        x_of(&app, fixed)
    );
    step(&mut app, 1.0);
    assert_eq!(x_of(&app, fixed), 110.0);
}

/// Becoming fixed (and back) moves the rect into another space — here from
/// 10px off its parent's right edge to 10px off the viewport's. There is no
/// old rect to ease from, so the node snaps and nothing is in flight.
#[test]
fn fixed_flip_snaps_the_layout_channel() {
    let mut app = layout_app();
    let root = app.world_mut().spawn(root_row()).id();
    app.world_mut().spawn((
        Node {
            width: Val::Px(200.0),
            ..square(100.0)
        },
        ChildOf(root),
    ));
    let parent = app.world_mut().spawn((square(100.0), ChildOf(root))).id();
    let node = app
        .world_mut()
        .spawn((
            Node {
                left: Val::Auto,
                right: Val::Px(10.0),
                ..pinned(0.0)
            },
            ChildOf(parent),
            layout_spec(1.0),
            TransitionState::default(),
        ))
        .id();
    step(&mut app, 0.016);
    assert_eq!(x_of(&app, node), 280.0, "against its parent");

    let in_flight = |app: &App| {
        let state = app.world().entity(node).get::<TransitionState>().unwrap();
        state.layout.in_flight()
    };
    app.world_mut().entity_mut(node).insert(FixedNode);
    step(&mut app, 0.0);
    assert_eq!(x_of(&app, node), 980.0, "against the viewport, at once");
    assert!(!in_flight(&app));

    app.world_mut().entity_mut(node).remove::<FixedNode>();
    step(&mut app, 0.0);
    assert_eq!(x_of(&app, node), 280.0, "and back");
    assert!(!in_flight(&app));

    // Inside one space it eases as ever.
    app.world_mut().get_mut::<Node>(node).unwrap().right = Val::Px(30.0);
    step(&mut app, 0.0);
    assert_eq!(x_of(&app, node), 280.0);
    assert!(in_flight(&app));
}

/// `layoutRounding` does not inherit into a fixed node (bevy's walk restarts
/// at rounding there): under an unrounded ancestor the channel still
/// measures the ROUNDED rect, the one bevy displayed.
#[test]
fn fixed_node_restarts_layout_rounding() {
    let mut app = layout_app();
    let root = app
        .world_mut()
        .spawn((
            root_row(),
            LayoutConfig {
                use_rounding: false,
            },
        ))
        .id();
    let fixed = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(33.3),
                ..pinned(10.4)
            },
            FixedNode,
            ChildOf(root),
            layout_spec(1.0),
            TransitionState::default(),
        ))
        .id();
    step(&mut app, 0.016);
    let shown = global(&app, fixed);
    assert_eq!(shown.translation.x, 27.0, "rounded: 10..44");

    // Unrounded this is a move of 89.9, rounded one of 90.
    app.world_mut().get_mut::<Node>(fixed).unwrap().left = Val::Px(100.3);
    step(&mut app, 0.0);
    let change = global(&app, fixed);
    assert!(
        (change.translation.x - 27.0).abs() < 1e-4 && change.matrix2 == shown.matrix2,
        "change frame shows the rect bevy displayed, got {change:?}"
    );
}

/// A shared-element seed on a fixed node: the seed frame shows the outgoing
/// rect where it was, at its size — the scaled parent's frame is not the
/// node's.
#[test]
fn shared_seed_lands_in_root_space_on_a_fixed_node() {
    let mut app = layout_app();
    app.init_resource::<crate::layer::LayerContentDirt>();
    app.add_systems(Update, drive_transitions);
    let root = app.world_mut().spawn(root_row()).id();
    let parent = app
        .world_mut()
        .spawn((
            square(100.0),
            UiTransform::from_scale(Vec2::splat(2.0)),
            ChildOf(root),
        ))
        .id();
    let fixed = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(200.0),
                height: Val::Px(50.0),
                ..pinned(0.0)
            },
            FixedNode,
            ChildOf(parent),
            TransitionInput {
                spec: Transition {
                    shared_element: Some(timing(1.0, Easing::Linear)),
                    ..Default::default()
                },
                width: Some(Length::Px(200.0)),
                height: Some(Length::Px(50.0)),
                ..Default::default()
            },
            TransitionState::default(),
            SharedSeed {
                state: Box::new(TransitionState::default()),
                rect: SharedRect {
                    center: Vec2::new(300.0, 50.0),
                    size: Vec2::new(100.0, 50.0),
                },
            },
        ))
        .id();
    step(&mut app, 0.0);
    let g = global(&app, fixed);
    assert_eq!(g.translation, Vec2::new(300.0, 50.0), "the seed's center");
    assert!(
        (g.matrix2.x_axis.x - 0.5).abs() < 1e-6 && (g.matrix2.y_axis.y - 1.0).abs() < 1e-6,
        "the seed's size through the FLIP scale, got {:?}",
        g.matrix2
    );
}
