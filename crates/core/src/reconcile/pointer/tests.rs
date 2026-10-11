use bevy::picking::events::Pointer;
use bevy::picking::pointer::PointerMap;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use tokio::sync::mpsc::UnboundedReceiver;

use super::*;
use crate::PointerCapture;
use crate::protocol::{op::Op, outbound::Outbound, outbound::UiEvent};
use crate::reconcile::picking::test_pointers::{at, hit, move_to, spawn_pointer};
use crate::touch_scroll::TouchScrollState;

const PRIMARY: PointerButton = PointerButton::Primary;
const MOUSE: PointerId = PointerId::Mouse;

/// A minimal app running [`collect_pointer_events`] on hand-written picking
/// messages, with a `JsBridge` whose outbound receiver is returned.
fn drag_app() -> (App, UnboundedReceiver<Outbound>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel::<Outbound>();
    let (ops_tx, ops_rx) = crossbeam_channel::unbounded::<Vec<Op>>();
    std::mem::forget(ops_tx); // Keep the ops channel open for the app's lifetime.
    let root = app.world_mut().spawn_empty().id();
    app.insert_resource(JsBridge::new(ops_rx, out_tx, root))
        .init_resource::<PointerMap>()
        .init_resource::<PointerCapture>()
        .init_resource::<ActiveDrag>()
        .init_resource::<TouchScrollState>()
        .add_message::<PointerPress>()
        .add_message::<PointerInput>()
        .add_systems(Update, collect_pointer_events);
    (app, out_rx)
}

const HANDLERS: PointerHandlers = PointerHandlers {
    down: true,
    moved: true,
    up: true,
    enter: false,
    leave: true,
    hover: false,
};

/// A handler node: a 200×200 box centered at (100, 100), scale factor 1.
fn handler_node(app: &mut App, id: u32) -> Entity {
    app.world_mut()
        .spawn((
            ReactNode(id),
            HANDLERS,
            ComputedNode {
                size: Vec2::splat(200.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::splat(100.0)),
        ))
        .id()
}

/// `pointer` presses `button` at `position`, landing on `entities` (topmost
/// first): the input, and the `PointerPress` bevy_picking sends each.
fn press(
    app: &mut App,
    pointer: PointerId,
    button: PointerButton,
    position: Vec2,
    entities: &[Entity],
) {
    let world = app.world_mut();
    world.write_message(PointerInput::new(
        pointer,
        at(position),
        PointerAction::Press(button),
    ));
    for (depth, &entity) in entities.iter().enumerate() {
        world.write_message(PointerPress {
            entity,
            pointer: Pointer::new(pointer, at(position)),
            button,
            hit: hit(depth as f32 * 0.00001),
            count: 1,
        });
    }
}

fn act(app: &mut App, pointer: PointerId, position: Vec2, action: PointerAction) {
    app.world_mut()
        .write_message(PointerInput::new(pointer, at(position), action));
}

fn release(app: &mut App, pointer: PointerId, button: PointerButton, position: Vec2) {
    act(app, pointer, position, PointerAction::Release(button));
}

/// Run a frame and return what it sent.
fn frame(app: &mut App, rx: &mut UnboundedReceiver<Outbound>) -> Vec<UiEvent> {
    app.update();
    std::iter::from_fn(|| rx.try_recv().ok())
        .map(|o| match o {
            Outbound::UiEvent { event } => event,
            other => panic!("expected a UiEvent, got {other:?}"),
        })
        .collect()
}

fn kinds(events: &[UiEvent]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

fn dragging(app: &App) -> bool {
    app.world().resource::<PointerCapture>().dragging
}

/// Down on the press, a move only when the pointer actually moved (a held,
/// still pointer is silent — the regression was one identical event per
/// frame), up on the release — with the position inside the node and the
/// pointer's own client position.
#[test]
fn a_drag_reports_down_move_up() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    let mouse = spawn_pointer(app.world_mut(), MOUSE, Vec2::splat(100.0));

    press(&mut app, MOUSE, PRIMARY, Vec2::splat(100.0), &[node]);
    let down = frame(&mut app, &mut rx);
    assert_eq!(kinds(&down), ["pointerDown"]);
    assert_eq!((down[0].x, down[0].y), (Some(0.5), Some(0.5)));
    assert_eq!((down[0].client_x, down[0].button), (Some(100.0), Some(0)));
    assert!(dragging(&app));

    assert!(frame(&mut app, &mut rx).is_empty(), "held but still");

    // Past the node's right edge: `x` clamps, the client position does not.
    move_to(app.world_mut(), mouse, Vec2::new(250.0, 100.0));
    let moved = frame(&mut app, &mut rx);
    assert_eq!(kinds(&moved), ["pointerMove"]);
    assert_eq!((moved[0].x, moved[0].client_x), (Some(1.0), Some(250.0)));

    release(&mut app, MOUSE, PRIMARY, Vec2::new(250.0, 100.0));
    assert_eq!(kinds(&frame(&mut app, &mut rx)), ["pointerUp"]);
    assert!(!dragging(&app));
    assert!(frame(&mut app, &mut rx).is_empty());
}

/// A pass-through stack presses several handler nodes at once; the topmost
/// (smallest depth) owns the gesture, whatever order the messages come in.
#[test]
fn the_topmost_pressed_handler_gets_the_drag() {
    let (mut app, mut rx) = drag_app();
    let top = handler_node(&mut app, 1);
    let beneath = handler_node(&mut app, 2);
    spawn_pointer(app.world_mut(), MOUSE, Vec2::splat(100.0));
    // A plain node on top of both is no candidate.
    let plain = app.world_mut().spawn(ReactNode(3)).id();

    let location = Pointer::new(MOUSE, at(Vec2::splat(100.0)));
    for (entity, depth) in [(beneath, 0.00002), (plain, 0.0), (top, 0.00001)] {
        app.world_mut().write_message(PointerPress {
            entity,
            pointer: location.clone(),
            button: PRIMARY,
            hit: hit(depth),
            count: 1,
        });
    }
    let down = frame(&mut app, &mut rx);
    assert_eq!(kinds(&down), ["pointerDown"]);
    assert_eq!(down[0].id, 1);
}

/// One drag per device: another button pressed meanwhile starts nothing and
/// its release ends nothing; the drag reports the button that began it.
#[test]
fn a_second_button_is_ignored_during_a_drag() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    let other = handler_node(&mut app, 2);
    spawn_pointer(app.world_mut(), MOUSE, Vec2::splat(100.0));
    let here = Vec2::splat(100.0);

    press(&mut app, MOUSE, PointerButton::Secondary, here, &[node]);
    let down = frame(&mut app, &mut rx);
    assert_eq!(
        (kinds(&down), down[0].button),
        (vec!["pointerDown"], Some(2))
    );

    press(&mut app, MOUSE, PRIMARY, here, &[other]);
    assert!(frame(&mut app, &mut rx).is_empty());
    release(&mut app, MOUSE, PRIMARY, here);
    assert!(frame(&mut app, &mut rx).is_empty());

    release(&mut app, MOUSE, PointerButton::Secondary, here);
    let up = frame(&mut app, &mut rx);
    assert_eq!(
        (kinds(&up), up[0].id, up[0].button),
        (vec!["pointerUp"], 1, Some(2))
    );
}

/// A finger is its own pointer: its drag reports the primary button and the
/// finger's position, never a mouse resting elsewhere — including on a
/// cancel (palm rejection, an OS gesture), which ends the drag where it says.
#[test]
fn a_touch_drags_at_its_own_position() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    spawn_pointer(app.world_mut(), MOUSE, Vec2::new(200.0, 50.0));
    let finger = PointerId::Touch(7);
    let finger_entity = spawn_pointer(app.world_mut(), finger, Vec2::splat(100.0));

    press(&mut app, finger, PRIMARY, Vec2::splat(100.0), &[node]);
    let down = frame(&mut app, &mut rx);
    assert_eq!(
        (kinds(&down), down[0].button),
        (vec!["pointerDown"], Some(0))
    );
    assert_eq!(down[0].client_x, Some(100.0));
    assert!(dragging(&app), "a touch drag claims the pointer");

    move_to(app.world_mut(), finger_entity, Vec2::new(110.0, 100.0));
    let moved = frame(&mut app, &mut rx);
    assert_eq!(
        (kinds(&moved), moved[0].client_x),
        (vec!["pointerMove"], Some(110.0))
    );

    act(
        &mut app,
        finger,
        Vec2::new(112.0, 100.0),
        PointerAction::Cancel,
    );
    let up = frame(&mut app, &mut rx);
    assert_eq!(
        (kinds(&up), up[0].client_x),
        (vec!["pointerUp"], Some(112.0))
    );
    assert!(!dragging(&app));
}

/// The touch-scroll arbitration steals a finger's drag once it pans along a
/// scrollable axis: the drag ends the next frame with `pointerLeave` (no
/// `pointerUp`, no further moves), and the finger's eventual release is
/// silent.
#[test]
fn a_stolen_touch_drag_ends_with_pointer_leave() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    let finger = PointerId::Touch(7);
    let finger_entity = spawn_pointer(app.world_mut(), finger, Vec2::splat(100.0));

    press(&mut app, finger, PRIMARY, Vec2::splat(100.0), &[node]);
    assert_eq!(kinds(&frame(&mut app, &mut rx)), ["pointerDown"]);

    // What `apply_touch_scroll` records once the pan crosses the slop.
    app.world_mut()
        .resource_mut::<TouchScrollState>()
        .steal_for_test(7);
    move_to(app.world_mut(), finger_entity, Vec2::new(100.0, 60.0));
    let leave = frame(&mut app, &mut rx);
    assert_eq!(kinds(&leave), ["pointerLeave"]);
    assert_eq!(leave[0].client_y, Some(60.0), "where the finger is");
    assert!(!dragging(&app), "a stolen drag no longer owns the pointer");

    move_to(app.world_mut(), finger_entity, Vec2::new(100.0, 40.0));
    assert!(frame(&mut app, &mut rx).is_empty());
    release(&mut app, finger, PRIMARY, Vec2::new(100.0, 40.0));
    assert!(frame(&mut app, &mut rx).is_empty());
}

/// A node despawned mid-drag keeps its binding until the release — silently.
#[test]
fn a_despawned_node_holds_the_pointer_until_release() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    let mouse = spawn_pointer(app.world_mut(), MOUSE, Vec2::splat(100.0));
    press(&mut app, MOUSE, PRIMARY, Vec2::splat(100.0), &[node]);
    frame(&mut app, &mut rx);

    app.world_mut().despawn(node);
    move_to(app.world_mut(), mouse, Vec2::splat(120.0));
    assert!(frame(&mut app, &mut rx).is_empty());
    assert!(dragging(&app));

    release(&mut app, MOUSE, PRIMARY, Vec2::splat(120.0));
    assert!(frame(&mut app, &mut rx).is_empty());
    assert!(!dragging(&app));
}

/// Every pointer runs its own drag.
#[test]
fn two_pointers_drag_two_nodes() {
    let (mut app, mut rx) = drag_app();
    let a = handler_node(&mut app, 1);
    let b = handler_node(&mut app, 2);
    spawn_pointer(app.world_mut(), MOUSE, Vec2::splat(100.0));
    let finger = PointerId::Touch(1);
    let finger_entity = spawn_pointer(app.world_mut(), finger, Vec2::splat(100.0));

    press(&mut app, MOUSE, PRIMARY, Vec2::splat(100.0), &[a]);
    press(&mut app, finger, PRIMARY, Vec2::splat(100.0), &[b]);
    let mut ids: Vec<_> = frame(&mut app, &mut rx).iter().map(|e| e.id).collect();
    ids.sort();
    assert_eq!(ids, [1, 2], "both went down");

    move_to(app.world_mut(), finger_entity, Vec2::splat(110.0));
    let moved = frame(&mut app, &mut rx);
    assert_eq!((kinds(&moved), moved[0].id), (vec!["pointerMove"], 2));

    release(&mut app, MOUSE, PRIMARY, Vec2::splat(100.0));
    let up = frame(&mut app, &mut rx);
    assert_eq!((kinds(&up), up[0].id), (vec!["pointerUp"], 1));
    assert!(dragging(&app), "the finger still drags");
}

/// The mouse and its transform3d remap are one device: when both press a
/// handler node in one frame, the remapped press wins (the quad is what the
/// user sees) whatever the depths, and reports the real cursor as the client
/// position. A mouse click is then one drag, not two.
#[test]
fn the_remapped_press_beats_the_mouse_press() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, mut rx) = drag_app();
    let on_quad = handler_node(&mut app, 1);
    let beneath = handler_node(&mut app, 2);
    app.world_mut()
        .run_system_once(crate::layer::pick3d::init_transform3d_pointer)
        .unwrap();
    let remap = app.world().resource::<Transform3dPointer>().id;
    let remap_entity = app.world().resource::<PointerMap>().get_entity(remap);
    let on_plane = Vec2::new(150.0, 100.0);
    move_to(app.world_mut(), remap_entity.unwrap(), on_plane);
    spawn_pointer(app.world_mut(), MOUSE, Vec2::new(640.0, 400.0));

    press(
        &mut app,
        MOUSE,
        PRIMARY,
        Vec2::new(640.0, 400.0),
        &[beneath],
    );
    // Deeper in its own stack than the mouse's hit is in its.
    act(&mut app, remap, on_plane, PointerAction::Press(PRIMARY));
    app.world_mut().write_message(PointerPress {
        entity: on_quad,
        pointer: Pointer::new(remap, at(on_plane)),
        button: PRIMARY,
        hit: hit(0.00002),
        count: 1,
    });
    let down = frame(&mut app, &mut rx);
    assert_eq!(kinds(&down), ["pointerDown"]);
    assert_eq!(down[0].id, 1, "the node on the quad");
    assert_eq!((down[0].x, down[0].client_x), (Some(0.75), Some(640.0)));

    // The mouse's own release is not the drag's; the remap's is.
    release(&mut app, MOUSE, PRIMARY, Vec2::new(640.0, 400.0));
    assert!(frame(&mut app, &mut rx).is_empty());
    release(&mut app, remap, PRIMARY, Vec2::new(150.0, 100.0));
    assert_eq!(kinds(&frame(&mut app, &mut rx)), ["pointerUp"]);
}

/// An element's own coordinate (an SVG shape's user space) wins over the
/// node-relative position while its `EventLocalPos` slot carries one. A
/// node-less element with an empty slot still drags: `x`/`y` stay at the
/// last reading (zero at first).
#[test]
fn an_elements_own_position_overrides_xy() {
    let (mut app, mut rx) = drag_app();
    let shape = app
        .world_mut()
        .spawn((
            ReactNode(1),
            HANDLERS,
            EventLocalPos(Some(Vec2::new(42.0, 17.0))),
        ))
        .id();
    let mouse = spawn_pointer(app.world_mut(), MOUSE, Vec2::splat(100.0));

    press(&mut app, MOUSE, PRIMARY, Vec2::splat(100.0), &[shape]);
    let down = frame(&mut app, &mut rx);
    assert_eq!((down[0].x, down[0].y), (Some(42.0), Some(17.0)));

    // The pointer left the shape: the feature cleared the slot.
    app.world_mut().get_mut::<EventLocalPos>(shape).unwrap().0 = None;
    move_to(app.world_mut(), mouse, Vec2::splat(300.0));
    let moved = frame(&mut app, &mut rx);
    assert_eq!(kinds(&moved), ["pointerMove"]);
    assert_eq!((moved[0].x, moved[0].y), (Some(42.0), Some(17.0)));
    assert_eq!(moved[0].client_x, Some(300.0));
}

/// A virtual pointer (a `<surface>`'s) drags like any other, but its drag is
/// not the window's: `PointerCapture::dragging` stays clear. When its driver
/// leaves — a release where the pointer was, then a park off-bounds — the
/// `pointerUp` reports the release, not the parking spot.
#[test]
fn a_virtual_pointer_drag_is_not_the_windows() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    let pointer = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(0x5eed));
    let entity = spawn_pointer(app.world_mut(), pointer, Vec2::splat(100.0));
    app.world_mut()
        .get_resource_or_init::<VirtualPointers>()
        .register(pointer);

    press(&mut app, pointer, PRIMARY, Vec2::splat(100.0), &[node]);
    assert_eq!(kinds(&frame(&mut app, &mut rx)), ["pointerDown"]);
    assert!(!dragging(&app));

    release(&mut app, pointer, PRIMARY, Vec2::new(150.0, 100.0));
    move_to(app.world_mut(), entity, Vec2::splat(-1.0));
    let up = frame(&mut app, &mut rx);
    assert_eq!(kinds(&up), ["pointerUp"]);
    assert_eq!((up[0].x, up[0].client_x), (Some(0.75), Some(150.0)));
}

/// A pointer that disappears mid-drag (its entity despawned without a
/// release) ends the drag where it last was.
#[test]
fn a_vanished_pointer_ends_the_drag_where_it_was() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    let finger = PointerId::Touch(3);
    let entity = spawn_pointer(app.world_mut(), finger, Vec2::splat(100.0));
    press(&mut app, finger, PRIMARY, Vec2::splat(100.0), &[node]);
    frame(&mut app, &mut rx);
    move_to(app.world_mut(), entity, Vec2::new(150.0, 100.0));
    frame(&mut app, &mut rx);

    app.world_mut().despawn(entity);
    let up = frame(&mut app, &mut rx);
    assert_eq!(kinds(&up), ["pointerUp"]);
    assert_eq!((up[0].x, up[0].client_x), (Some(0.75), Some(150.0)));
    assert!(!dragging(&app));
}

/// A whole click inside one frame is a down and an up. A release that came
/// *before* the press in the same frame ended some earlier gesture: the
/// drag the press begins stays.
#[test]
fn inputs_within_one_frame_keep_their_order() {
    let (mut app, mut rx) = drag_app();
    let node = handler_node(&mut app, 1);
    spawn_pointer(app.world_mut(), MOUSE, Vec2::splat(100.0));
    let here = Vec2::splat(100.0);

    press(&mut app, MOUSE, PRIMARY, here, &[node]);
    release(&mut app, MOUSE, PRIMARY, here);
    assert_eq!(
        kinds(&frame(&mut app, &mut rx)),
        ["pointerDown", "pointerUp"]
    );
    assert!(!dragging(&app));

    release(&mut app, MOUSE, PRIMARY, here);
    press(&mut app, MOUSE, PRIMARY, here, &[node]);
    assert_eq!(kinds(&frame(&mut app, &mut rx)), ["pointerDown"]);
    assert!(dragging(&app));
}

/// [`collect_pointer_events`] is the frame's `wheel_captured` assigner: a
/// claim from last frame's wheel systems must be reset so a single scrolled
/// frame doesn't trap world wheel-consumers forever.
#[test]
fn the_wheel_claim_resets_each_frame() {
    let (mut app, _rx) = drag_app();
    app.world_mut()
        .resource_mut::<PointerCapture>()
        .wheel_captured = true;
    app.update();
    assert!(!app.world().resource::<PointerCapture>().wheel_captured);
}
