//! What bevy does with a `FixedNode` (the facts `positionType: "fixed"` is
//! built on), and what [`sync_fixed_nodes`] adds. Real layout and clipping:
//! the harness of `transition::layout_tests`, a 1000×100 target at scale 1.

use super::*;
use crate::transition::layout_tests::layout_app;
use bevy::ui::update::{propagate_ui_target_cameras, update_clipping_system};
use bevy::ui::{CalculatedClip, UiGlobalTransform};

/// [`layout_app`] with the sync ahead of layout and bevy's clipping after
/// it, and a viewport-filling root.
fn fixed_app() -> (App, Entity) {
    let mut app = layout_app();
    app.add_systems(
        PostUpdate,
        (
            sync_fixed_nodes.before(propagate_ui_target_cameras),
            update_clipping_system.after(bevy::ui::ui_layout_system),
        ),
    );
    let root = app.world_mut().spawn(viewport()).id();
    (app, root)
}

fn viewport() -> Node {
    Node {
        width: Val::Vw(100.0),
        height: Val::Vh(100.0),
        ..default()
    }
}

/// An absolute 20×20 node; the insets are the caller's.
fn pin() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Px(20.0),
        height: Val::Px(20.0),
        ..default()
    }
}

/// A React node asking to be fixed, as the op path leaves it.
fn request(id: u32, node: Node, parent: Entity) -> impl Bundle {
    (ReactNode(id), PositionFixed, node, ChildOf(parent))
}

fn center(app: &App, e: Entity) -> Vec2 {
    app.world().get::<UiGlobalTransform>(e).unwrap().translation
}

fn size(app: &App, e: Entity) -> Vec2 {
    app.world().get::<ComputedNode>(e).unwrap().size
}

fn is_fixed(app: &App, e: Entity) -> bool {
    app.world().entity(e).contains::<FixedNode>()
}

/// The request becomes `FixedNode` in the frame it appears, and the node is
/// placed against the viewport: its parent's place, scroll offset and
/// transform do not reach it.
#[test]
fn fixed_node_ignores_parent_offset_scroll_and_transform() {
    let (mut app, root) = fixed_app();
    let port = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(200.0),
                height: Val::Px(80.0),
                margin: UiRect::left(Val::Px(50.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, 40.0)),
            UiTransform::from_scale(Vec2::splat(2.0)),
            ChildOf(root),
        ))
        .id();
    app.world_mut().spawn((
        Node {
            width: Val::Px(50.0),
            height: Val::Px(300.0),
            ..default()
        },
        ChildOf(port),
    ));
    let pinned = Node {
        right: Val::Px(10.0),
        bottom: Val::Px(10.0),
        ..pin()
    };
    let fixed = app.world_mut().spawn(request(1, pinned, port)).id();
    app.update();
    assert!(is_fixed(&app, fixed), "granted before this frame's layout");
    let at = Vec2::new(980.0, 80.0);
    assert_eq!(center(&app, fixed), at, "the viewport's bottom right");
    let global = **app.world().get::<UiGlobalTransform>(fixed).unwrap();
    assert_eq!(global.matrix2, Mat2::IDENTITY, "unscaled");

    let mut port = app.world_mut().entity_mut(port);
    port.insert((
        ScrollPosition(Vec2::ZERO),
        UiTransform::from_translation(Val2::px(30.0, 5.0)),
    ));
    port.get_mut::<Node>().unwrap().margin = UiRect::left(Val::Px(400.0));
    app.update();
    assert_eq!(center(&app, fixed), at);
}

/// A fixed child is not in its parent's content: the scroll range is the
/// same with and without it.
#[test]
fn fixed_child_leaves_the_parents_content_size() {
    let (mut app, root) = fixed_app();
    let port = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(200.0),
                height: Val::Px(80.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    app.world_mut().spawn((
        Node {
            width: Val::Px(50.0),
            height: Val::Px(300.0),
            flex_shrink: 0.0,
            ..default()
        },
        ChildOf(port),
    ));
    app.update();
    let content = |app: &App| app.world().get::<ComputedNode>(port).unwrap().content_size;
    let alone = content(&app);
    assert_eq!(alone.y, 300.0);

    let tall = Node {
        height: Val::Px(5000.0),
        top: Val::Px(0.0),
        ..pin()
    };
    app.world_mut().spawn(request(1, tall, port));
    app.update();
    assert_eq!(content(&app), alone);
}

/// Insets, percentages and stretching resolve against the viewport; with no
/// inset on an axis the node sits at the viewport's start, or where its own
/// `justifySelf`/`alignSelf` puts it.
#[test]
fn fixed_node_resolves_against_the_viewport() {
    let (mut app, root) = fixed_app();
    let parent = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(100.0),
                height: Val::Px(50.0),
                margin: UiRect::all(Val::Px(30.0)),
                justify_content: JustifyContent::End,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let mut spawn = |id, node| app.world_mut().spawn(request(id, node, parent)).id();
    let percent = spawn(
        1,
        Node {
            width: Val::Percent(50.0),
            left: Val::Percent(10.0),
            ..pin()
        },
    );
    let stretched = spawn(
        2,
        Node {
            width: Val::Auto,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..pin()
        },
    );
    let unset = spawn(3, pin());
    let centered = spawn(
        4,
        Node {
            justify_self: JustifySelf::Center,
            align_self: AlignSelf::End,
            ..pin()
        },
    );
    app.update();
    assert_eq!(size(&app, percent).x, 500.0);
    assert_eq!(center(&app, percent).x, 350.0, "100 + 500 / 2");
    assert_eq!(size(&app, stretched).x, 1000.0);
    assert_eq!(center(&app, unset), Vec2::splat(10.0), "top left");
    assert_eq!(center(&app, centered), Vec2::new(500.0, 90.0));
}

/// No ancestor clip reaches a fixed node or its subtree.
#[test]
fn fixed_node_escapes_ancestor_clip() {
    let (mut app, root) = fixed_app();
    let clipper = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(50.0),
                height: Val::Px(50.0),
                overflow: Overflow::clip(),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let plain = app.world_mut().spawn((pin(), ChildOf(clipper))).id();
    let fixed = app.world_mut().spawn(request(1, pin(), clipper)).id();
    let child = app.world_mut().spawn((pin(), ChildOf(fixed))).id();
    app.update();
    app.update();
    let clipped = |e: Entity| app.world().entity(e).contains::<CalculatedClip>();
    assert!(clipped(plain));
    assert!(!clipped(fixed) && !clipped(child));
}

/// Why [`sync_fixed_nodes`] looks at ancestors: bevy alone keeps a fixed
/// node laid out and unclipped under a `display: none` parent. (If this ever
/// fails, bevy hides it itself and the ancestor check can go.)
#[test]
fn bevy_fixed_node_survives_a_display_none_parent() {
    let (mut app, root) = fixed_app();
    let hidden = app
        .world_mut()
        .spawn((
            Node {
                display: Display::None,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    // Not a React node: the sync leaves it alone.
    let fixed = app
        .world_mut()
        .spawn((pin(), FixedNode, ChildOf(hidden)))
        .id();
    app.update();
    app.update();
    assert_eq!(size(&app, fixed), Vec2::splat(20.0));
    assert!(!app.world().entity(fixed).contains::<CalculatedClip>());
    assert!(is_fixed(&app, fixed));
}

/// A `display: none` ancestor withdraws `FixedNode` — the node is then a
/// hidden absolute child like any other — and showing the ancestor again
/// restores it, each within the frame of the change.
#[test]
fn display_none_ancestor_unfixes_and_restores() {
    let (mut app, root) = fixed_app();
    let grandparent = app.world_mut().spawn((viewport(), ChildOf(root))).id();
    let parent = app
        .world_mut()
        .spawn((viewport(), ChildOf(grandparent)))
        .id();
    let fixed = app.world_mut().spawn(request(1, pin(), parent)).id();
    app.update();
    assert!(is_fixed(&app, fixed));
    assert_eq!(size(&app, fixed), Vec2::splat(20.0));

    for ancestor in [parent, grandparent] {
        let display = |app: &mut App, display| {
            app.world_mut().get_mut::<Node>(ancestor).unwrap().display = display;
            app.update();
        };
        display(&mut app, Display::None);
        assert!(!is_fixed(&app, fixed));
        assert_eq!(size(&app, fixed), Vec2::ZERO, "hidden with its ancestor");
        display(&mut app, Display::Flex);
        assert!(is_fixed(&app, fixed));
        assert_eq!(size(&app, fixed), Vec2::splat(20.0));
    }

    // Mounted straight into a hidden subtree: never fixed.
    app.world_mut().get_mut::<Node>(parent).unwrap().display = Display::None;
    let late = app.world_mut().spawn(request(2, pin(), parent)).id();
    app.update();
    assert!(!is_fixed(&app, late) && !is_fixed(&app, fixed));
    // ...and attached to one after the fact (no `Node` changes).
    let loose = app
        .world_mut()
        .spawn((ReactNode(3), PositionFixed, pin()))
        .id();
    app.update();
    app.world_mut().entity_mut(loose).insert(ChildOf(parent));
    app.update();
    assert!(!is_fixed(&app, loose));
}

/// Dropping the request removes the component; nothing else of ours ever
/// carries one.
#[test]
fn dropping_the_request_removes_fixed_node() {
    let (mut app, root) = fixed_app();
    let fixed = app.world_mut().spawn(request(1, pin(), root)).id();
    app.update();
    assert!(is_fixed(&app, fixed));

    app.world_mut().entity_mut(fixed).remove::<PositionFixed>();
    app.update();
    assert!(!is_fixed(&app, fixed));

    // Bridge-owned: one the style did not ask for goes at the next sync.
    app.world_mut().entity_mut(fixed).insert(FixedNode);
    app.world_mut().spawn(request(2, pin(), root));
    app.update();
    assert!(!is_fixed(&app, fixed));
}

/// The viewport is the node's own UI camera's: under a root that targets
/// another camera (a `<surface>`, a `<root>` on a second window) a fixed
/// node is placed against THAT target.
#[test]
fn fixed_node_uses_its_roots_camera_viewport() {
    use bevy::camera::{Camera, ComputedCameraValues, RenderTargetInfo, Viewport};
    let (mut app, _) = fixed_app();
    let target = UVec2::new(400, 300);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: target,
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                viewport: Some(Viewport {
                    physical_size: target,
                    ..default()
                }),
                ..default()
            },
        ))
        .id();
    let surface = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(100.0),
                height: Val::Px(100.0),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .id();
    let pinned = Node {
        right: Val::Px(0.0),
        bottom: Val::Px(0.0),
        ..pin()
    };
    let fixed = app.world_mut().spawn(request(1, pinned, surface)).id();
    app.update();
    assert_eq!(center(&app, fixed), Vec2::new(390.0, 290.0));
}
