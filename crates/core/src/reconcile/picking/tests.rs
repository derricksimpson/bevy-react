use super::test_pointers::{hold, hover, press, spawn_pointer};
use super::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::picking::Pickable;
use bevy::picking::backend::HitData;
use bevy::picking::pointer::PointerMap;

use crate::layer::pick3d::TRANSFORM3D_POINTER_UUID;
use crate::reconcile::clip_pick_tests::{mouse_at, picking_app};

/// A hit on a React run (direct, or nested in another run) names its
/// `<text>` block afterwards; a foreign `TextSpan` and a plain node keep
/// their hits.
#[test]
fn span_hit_folds_onto_its_text_block() {
    let mut world = World::new();
    world.init_resource::<Messages<PointerHits>>();
    let block = world.spawn((Text::default(), ReactNode(1))).id();
    let run = world
        .spawn((TextSpan::default(), ReactNode(2), ChildOf(block)))
        .id();
    let nested = world
        .spawn((TextSpan::default(), ReactNode(3), ChildOf(run)))
        .id();
    let foreign_block = world.spawn(Text::default()).id();
    let foreign = world
        .spawn((TextSpan::default(), ChildOf(foreign_block)))
        .id();
    let node = world.spawn(ReactNode(4)).id();

    let hit = || HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
    world
        .resource_mut::<Messages<PointerHits>>()
        .write(PointerHits::new(
            PointerId::Mouse,
            vec![
                (run, hit()),
                (nested, hit()),
                (foreign, hit()),
                (node, hit()),
            ],
            0.5,
        ));
    world.run_system_once(fold_span_hits).unwrap();

    let picks: Vec<Entity> = world
        .resource_mut::<Messages<PointerHits>>()
        .drain()
        .flat_map(|hits| hits.picks.into_iter().map(|(entity, _)| entity))
        .collect();
    assert_eq!(picks, [block, block, foreign, node]);
}

/// How many `ReactInteraction`s the last update wrote.
#[derive(Resource, Default)]
struct Writes(usize);

/// An app running [`sync_interactions`] with a mouse pointer and a write
/// counter.
fn sync_app() -> App {
    let mut app = App::new();
    app.init_resource::<PointerMap>()
        .init_resource::<HoverMap>()
        .init_resource::<PointerState>()
        .init_resource::<Writes>()
        .add_systems(
            Update,
            (
                sync_interactions,
                |changed: Query<(), Changed<ReactInteraction>>, mut writes: ResMut<Writes>| {
                    writes.0 = changed.iter().count();
                },
            )
                .chain(),
        );
    spawn_pointer(app.world_mut(), PointerId::Mouse, Vec2::ZERO);
    app
}

fn state(app: &App, entity: Entity) -> ReactInteraction {
    *app.world().get::<ReactInteraction>(entity).unwrap()
}

fn writes(app: &App) -> usize {
    app.world().resource::<Writes>().0
}

#[test]
fn hover_then_press_then_none() {
    let mut app = sync_app();
    let node = app.world_mut().spawn(ReactInteraction::None).id();
    // Named by the maps, but not ours to write.
    let foreign = app.world_mut().spawn_empty().id();
    app.update(); // The spawn's own change tick.

    hover(app.world_mut(), PointerId::Mouse, &[node, foreign]);
    app.update();
    assert_eq!(state(&app, node), ReactInteraction::Hovered);
    app.update();
    assert_eq!(writes(&app), 0, "a steady hover writes nothing");

    press(
        app.world_mut(),
        PointerId::Mouse,
        PointerButton::Primary,
        &[node, foreign],
    );
    app.update();
    assert_eq!(state(&app, node), ReactInteraction::Pressed);
    app.update();
    assert_eq!(writes(&app), 0, "a held press writes nothing");

    press(
        app.world_mut(),
        PointerId::Mouse,
        PointerButton::Primary,
        &[],
    );
    hover(app.world_mut(), PointerId::Mouse, &[]);
    app.update();
    assert_eq!(state(&app, node), ReactInteraction::None);
    assert_eq!(writes(&app), 1);
    app.update();
    assert_eq!(writes(&app), 0, "an idle frame writes nothing");
}

/// A disabled control is hovered but never pressed: held down it reads
/// `Hovered` while the pointer is on it and `None` once it left. It still
/// counts as UI under the pointer.
#[test]
fn a_disabled_node_is_never_pressed() {
    let mut app = sync_app();
    app.init_resource::<crate::PointerCapture>();
    let node = app
        .world_mut()
        .spawn((ReactInteraction::None, bevy::ui::InteractionDisabled))
        .id();
    hover(app.world_mut(), PointerId::Mouse, &[node]);
    press(
        app.world_mut(),
        PointerId::Mouse,
        PointerButton::Primary,
        &[node],
    );
    app.update();
    assert_eq!(state(&app, node), ReactInteraction::Hovered);
    assert!(app.world().resource::<crate::PointerCapture>().over_ui);

    hover(app.world_mut(), PointerId::Mouse, &[]);
    app.update();
    assert_eq!(state(&app, node), ReactInteraction::None);
}

#[test]
fn press_is_sticky_until_release() {
    let mut app = sync_app();
    let node = app.world_mut().spawn(ReactInteraction::None).id();
    let other = app.world_mut().spawn(ReactInteraction::None).id();

    // Pressed, then dragged off: no longer in the hover map.
    press(
        app.world_mut(),
        PointerId::Mouse,
        PointerButton::Primary,
        &[node],
    );
    // The right button presses nothing.
    press(
        app.world_mut(),
        PointerId::Mouse,
        PointerButton::Secondary,
        &[other],
    );
    app.update();
    assert_eq!(state(&app, node), ReactInteraction::Pressed);
    assert_eq!(state(&app, other), ReactInteraction::None);
}

#[test]
fn every_pointer_counts() {
    let mut app = sync_app();
    let touched = app.world_mut().spawn(ReactInteraction::None).id();
    let lifted = app.world_mut().spawn(ReactInteraction::None).id();
    let custom = app.world_mut().spawn(ReactInteraction::None).id();

    // A finger that is down hovers what is under it.
    spawn_pointer(app.world_mut(), PointerId::Touch(7), Vec2::ZERO);
    hold(app.world_mut(), PointerId::Touch(7));
    hover(app.world_mut(), PointerId::Touch(7), &[touched]);
    // A finger that was lifted this frame is still in the map: no hover.
    spawn_pointer(app.world_mut(), PointerId::Touch(8), Vec2::ZERO);
    hover(app.world_mut(), PointerId::Touch(8), &[lifted]);
    // A feature's pointer (a `<surface>`'s) hovers like any other.
    let id = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(0x5eed));
    spawn_pointer(app.world_mut(), id, Vec2::ZERO);
    hover(app.world_mut(), id, &[custom]);

    app.update();
    assert_eq!(state(&app, touched), ReactInteraction::Hovered);
    assert_eq!(state(&app, lifted), ReactInteraction::None);
    assert_eq!(state(&app, custom), ReactInteraction::Hovered);
}

/// bevy_picking keeps the mouse where it left the window, so the hover
/// map keeps naming the node under that spot: not hovered. A press held
/// across the edge stays a press.
#[test]
fn a_left_window_drops_mouse_hover() {
    let mut app = App::new();
    app.init_resource::<PointerMap>()
        .add_systems(Update, sync_interactions);
    // A window without a cursor position, and the mouse still "on" it.
    let window = app.world_mut().spawn(Window::default()).id();
    let on_window = bevy::picking::pointer::Location {
        target: NormalizedRenderTarget::Window(
            bevy::window::WindowRef::Entity(window)
                .normalize(None)
                .unwrap(),
        ),
        position: Vec2::new(10.0, 10.0),
    };
    app.world_mut()
        .spawn((PointerId::Mouse, PointerLocation::new(on_window)));
    let remap = PointerId::Custom(TRANSFORM3D_POINTER_UUID);
    app.world_mut()
        .run_system_once(crate::layer::pick3d::init_transform3d_pointer)
        .unwrap();
    spawn_pointer(app.world_mut(), PointerId::Touch(1), Vec2::ZERO);

    let under_mouse = app.world_mut().spawn(ReactInteraction::None).id();
    let under_remap = app.world_mut().spawn(ReactInteraction::None).id();
    let held = app.world_mut().spawn(ReactInteraction::None).id();
    hover(app.world_mut(), PointerId::Mouse, &[under_mouse, held]);
    hover(app.world_mut(), remap, &[under_remap]);
    press(
        app.world_mut(),
        PointerId::Mouse,
        PointerButton::Primary,
        &[held],
    );
    app.update();
    assert_eq!(state(&app, under_mouse), ReactInteraction::None);
    assert_eq!(state(&app, under_remap), ReactInteraction::None);
    assert_eq!(state(&app, held), ReactInteraction::Pressed);

    // The cursor is back.
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(10.0, 10.0)));
    app.update();
    assert_eq!(state(&app, under_mouse), ReactInteraction::Hovered);
    assert_eq!(state(&app, under_remap), ReactInteraction::Hovered);
}

/// The state is read off the component, not a private diff: a node that
/// becomes interactive under a resting pointer is hovered next frame.
#[test]
fn a_component_gained_while_hovered_catches_up() {
    let mut app = sync_app();
    let node = app.world_mut().spawn_empty().id();
    hover(app.world_mut(), PointerId::Mouse, &[node]);
    app.update();
    app.world_mut()
        .entity_mut(node)
        .insert(ReactInteraction::None);
    app.update();
    assert_eq!(state(&app, node), ReactInteraction::Hovered);
}

/// A parent with a child on top of it, the mouse over both, through real
/// layout, bevy_ui's picking backend and the hover map.
fn stacked(child_blocks: bool) -> ReactInteraction {
    let (mut app, window) = picking_app();
    app.add_systems(
        PreUpdate,
        sync_interactions.after(bevy::picking::PickingSystems::Hover),
    );
    mouse_at(app.world_mut(), window, Vec2::new(50.0, 50.0));
    let world = app.world_mut();
    let size = Node {
        width: Val::Px(100.0),
        height: Val::Px(100.0),
        ..Default::default()
    };
    let pass = Pickable {
        should_block_lower: false,
        is_hoverable: true,
    };
    let child = world
        .spawn((
            size.clone(),
            InheritedVisibility::VISIBLE,
            Pickable {
                should_block_lower: child_blocks,
                is_hoverable: true,
            },
        ))
        .id();
    let parent = world
        .spawn((
            size,
            InheritedVisibility::VISIBLE,
            pass,
            ReactInteraction::None,
        ))
        .add_child(child)
        .id();
    world.insert_resource(bevy::ui::UiStack {
        uinodes: vec![parent, child],
        partition: std::iter::once(0..2).collect(),
    });
    app.update();
    app.update();
    state(&app, parent)
}

/// The stack rule `focusPolicy` documents, pinned against stock picking.
#[test]
fn a_blocking_child_unhovers_what_is_beneath() {
    assert_eq!(stacked(true), ReactInteraction::None);
}

#[test]
fn a_passing_child_keeps_its_parent_hovered() {
    assert_eq!(stacked(false), ReactInteraction::Hovered);
}

/// `PointerCapture::over_ui` answers for the window: a virtual pointer over
/// its own UI (a `<surface>`'s) doesn't claim it, a window pointer does — and
/// the claim goes with the hover.
#[test]
fn over_ui_counts_window_pointers_only() {
    let mut app = sync_app();
    app.init_resource::<crate::PointerCapture>();
    let virtual_id = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(0x5eed));
    spawn_pointer(app.world_mut(), virtual_id, Vec2::ZERO);
    app.world_mut()
        .get_resource_or_init::<crate::ext::VirtualPointers>()
        .register(virtual_id);
    let on_surface = app.world_mut().spawn(ReactInteraction::None).id();
    let on_window = app.world_mut().spawn(ReactInteraction::None).id();
    let over_ui = |app: &App| app.world().resource::<crate::PointerCapture>().over_ui;

    hover(app.world_mut(), virtual_id, &[on_surface]);
    app.update();
    assert_eq!(state(&app, on_surface), ReactInteraction::Hovered);
    assert!(!over_ui(&app));

    hover(app.world_mut(), PointerId::Mouse, &[on_window]);
    app.update();
    assert!(over_ui(&app));

    hover(app.world_mut(), PointerId::Mouse, &[]);
    app.update();
    assert!(!over_ui(&app));
}
