//! The press rule against bevy's own chain: real layout, bevy_ui's picking
//! backend, the hover map, `pointer_events` and stock pointer focus
//! (`PointerFocusPlugin`), driven by `PointerInput` messages. These fail
//! loudly if a future Bevy moves the pathway the rule replaces.

use bevy::ecs::system::RunSystemOnce;
use bevy::input_focus::pointer_focus::PointerFocusPlugin;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{FocusGained, FocusLost, InputFocus, InputFocusPlugin};
use bevy::picking::Pickable;
use bevy::picking::pointer::{
    Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
};
use bevy::prelude::*;
use bevy::ui::UiStack;
use bevy::window::WindowRef;

use crate::bridge::ReactNode;
use crate::reconcile::clip_pick_tests::picking_app;

const PASS: Pickable = Pickable {
    should_block_lower: false,
    is_hoverable: true,
};
const BLOCK: Pickable = Pickable {
    should_block_lower: true,
    is_hoverable: true,
};

/// What picking sees of an `<editableText>` at the default `focusPolicy`.
fn field() -> impl Bundle {
    (
        crate::style::writers::focus_pickable(
            crate::elements::editable::EDITABLE_TEXT.flags,
            bevy::ui::FocusPolicy::Pass,
        ),
        TabIndex(0),
    )
}

/// The `FocusGained`/`FocusLost` bevy sent, on the entity itself.
#[derive(Resource, Default)]
struct Log(Vec<(&'static str, Entity)>);

struct Scene {
    app: App,
    window: Entity,
    /// A window-filling container every box is a child of (the UI root).
    root: Entity,
    /// Back to front.
    stack: Vec<Entity>,
}

impl Scene {
    fn new() -> Self {
        let (mut app, window) = picking_app();
        app.add_plugins((InputFocusPlugin, PointerFocusPlugin));
        super::add_press_rule(&mut app);
        app.init_resource::<Log>();
        app.add_observer(|ev: On<FocusGained>, mut log: ResMut<Log>| {
            if ev.entity == ev.original_event_target() {
                log.0.push(("gained", ev.entity));
            }
        });
        app.add_observer(|ev: On<FocusLost>, mut log: ResMut<Log>| {
            if ev.entity == ev.original_event_target() {
                log.0.push(("lost", ev.entity));
            }
        });
        let root = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Vw(100.0),
                    height: Val::Vh(100.0),
                    ..Default::default()
                },
                InheritedVisibility::VISIBLE,
                Pickable::IGNORE,
            ))
            .id();
        Self {
            app,
            window,
            root,
            stack: vec![root],
        }
    }

    /// A 100×100 box at `x`, drawn above everything spawned before it.
    fn boxed(&mut self, x: f32, extra: impl Bundle) -> Entity {
        let entity = self
            .app
            .world_mut()
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(0.0),
                    width: Val::Px(100.0),
                    height: Val::Px(100.0),
                    ..Default::default()
                },
                InheritedVisibility::VISIBLE,
                ChildOf(self.root),
                extra,
            ))
            .id();
        self.stack.push(entity);
        entity
    }

    /// A React node (see [`Self::boxed`]).
    fn node(&mut self, x: f32, extra: impl Bundle) -> Entity {
        let id = self.stack.len() as u32;
        self.boxed(x, (ReactNode(id), extra))
    }

    /// Put pointer `id` on the window at `x` (mid-height).
    fn pointer(&mut self, id: PointerId, x: f32) {
        let location = self.at(x);
        self.app
            .world_mut()
            .spawn((id, PointerLocation::new(location)));
    }

    /// Move pointer `id` to `x`, and pick there.
    fn move_to(&mut self, id: PointerId, x: f32) {
        let input = PointerInput::new(id, self.at(x), PointerAction::Move { delta: Vec2::X });
        self.app.world_mut().write_message(input);
        self.app.update();
    }

    /// The pointer of 3D-transformed layers, on the window at `x`.
    fn remapped(&mut self, x: f32) -> PointerId {
        let id = PointerId::Custom(crate::layer::pick3d::TRANSFORM3D_POINTER_UUID);
        let world = self.app.world_mut();
        world
            .run_system_once(crate::layer::pick3d::init_transform3d_pointer)
            .unwrap();
        let pointer = world
            .query::<(Entity, &PointerId)>()
            .iter(world)
            .find_map(|(entity, pointer)| (*pointer == id).then_some(entity))
            .unwrap();
        let location = self.at(x);
        self.app
            .world_mut()
            .entity_mut(pointer)
            .insert(PointerLocation::new(location));
        id
    }

    fn at(&self, x: f32) -> Location {
        Location {
            target: bevy::camera::NormalizedRenderTarget::Window(
                WindowRef::Entity(self.window).normalize(None).unwrap(),
            ),
            position: Vec2::new(x, 50.0),
        }
    }

    /// Lay out, pick once, and forget the startup focus events.
    fn settle(&mut self) {
        let len = self.stack.len();
        self.app.world_mut().insert_resource(UiStack {
            uinodes: self.stack.clone(),
            partition: std::iter::once(0..len).collect(),
        });
        self.app.update();
        self.app.update();
        self.app.world_mut().resource_mut::<Log>().0.clear();
    }

    /// Press and release `pointers` (in this order, within one frame each);
    /// the focus events of both frames.
    fn click(&mut self, pointers: &[PointerId]) -> Vec<(&'static str, Entity)> {
        for action in [
            PointerAction::Press(PointerButton::Primary),
            PointerAction::Release(PointerButton::Primary),
        ] {
            for &pointer in pointers {
                // The location of an input only matters for moves.
                let input = PointerInput::new(pointer, self.at(0.0), action);
                self.app.world_mut().write_message(input);
            }
            self.app.update();
        }
        std::mem::take(&mut self.app.world_mut().resource_mut::<Log>().0)
    }

    fn focus(&self) -> Option<Entity> {
        self.app.world().resource::<InputFocus>().get()
    }
}

/// The case stock pointer focus loses: a pass-through field inside a
/// pass-through parent. One press lands on the field, the parent and the
/// window; bevy asks for focus once per pressed entity, in hash order, and
/// every request that reaches the window clears the focus.
#[test]
fn a_pass_through_field_keeps_its_focus() {
    let mut scene = Scene::new();
    let parent = scene.node(0.0, PASS);
    let field = scene.node(0.0, field());
    scene
        .app
        .world_mut()
        .entity_mut(field)
        .insert(ChildOf(parent));
    scene.pointer(PointerId::Mouse, 50.0);
    scene.settle();

    let log = scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(field));
    assert_eq!(log, [("lost", scene.window), ("gained", field)]);

    let log = scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(field));
    assert_eq!(log, [], "a re-click blurred the field");
}

/// A field under a pass-through overlay (a sibling drawn above it, not an
/// ancestor): the overlay is pressed too, and its request for focus reaches
/// the window.
#[test]
fn a_field_beneath_a_passing_overlay_takes_the_focus() {
    let mut scene = Scene::new();
    let field = scene.node(0.0, field());
    scene.node(0.0, PASS);
    scene.pointer(PointerId::Mouse, 50.0);
    scene.settle();

    scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(field));
    let log = scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(field));
    assert_eq!(log, [], "a re-click blurred the field");
}

/// One physical click through two pointers: the mouse presses whatever is
/// under the cursor on the window (at least the window), the pointer
/// remapped into a 3D-transformed layer presses the field. Stock focus
/// blurs and refocuses the field on every click — or, in the other input
/// order, leaves it blurred.
#[test]
fn a_click_through_two_pointers_focuses_once() {
    let mut scene = Scene::new();
    let field = scene.node(0.0, field());
    let remapped = scene.remapped(50.0);
    scene.pointer(PointerId::Mouse, 300.0);
    scene.settle();

    let log = scene.click(&[PointerId::Mouse, remapped]);
    assert_eq!(scene.focus(), Some(field));
    assert_eq!(log, [("lost", scene.window), ("gained", field)]);
    for order in [[PointerId::Mouse, remapped], [remapped, PointerId::Mouse]] {
        let log = scene.click(&order);
        assert_eq!(scene.focus(), Some(field), "pressed as {order:?}");
        assert_eq!(log, [], "pressed as {order:?}");
    }
}

/// The innermost focusable under the pointer wins, through a label that is
/// not focusable itself.
#[test]
fn the_topmost_resolving_press_wins() {
    let mut scene = Scene::new();
    let outer = scene.node(0.0, (PASS, TabIndex(0)));
    let inner = scene.node(0.0, (PASS, TabIndex(0)));
    let label = scene.node(0.0, PASS);
    let world = scene.app.world_mut();
    world.entity_mut(inner).insert(ChildOf(outer));
    world.entity_mut(label).insert(ChildOf(inner));
    scene.pointer(PointerId::Mouse, 50.0);
    scene.settle();

    let log = scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(inner));
    assert_eq!(log, [("lost", scene.window), ("gained", inner)]);
}

/// A press on React UI with nothing focusable under it blurs, once.
#[test]
fn a_press_that_resolves_nowhere_clears() {
    let mut scene = Scene::new();
    let field = scene.node(0.0, field());
    scene.node(200.0, PASS);
    scene.pointer(PointerId::Mouse, 50.0);
    scene.settle();
    scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(field));

    scene.move_to(PointerId::Mouse, 250.0);
    let log = scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), None);
    assert_eq!(log, [("lost", field)]);
}

/// Presses that touch no React node are bevy's: the app's own widget takes
/// the focus, a click on the bare window clears it. And the app's widget
/// beneath pass-through React UI is focused by the same rule as ours.
#[test]
fn presses_off_react_ui_are_left_to_bevy() {
    let mut scene = Scene::new();
    let widget = scene.boxed(0.0, (BLOCK, TabIndex(0)));
    let covered = scene.boxed(200.0, (BLOCK, TabIndex(0)));
    scene.node(200.0, PASS);
    scene.pointer(PointerId::Mouse, 50.0);
    scene.settle();

    scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(widget));

    scene.move_to(PointerId::Mouse, 250.0);
    scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(covered));
    assert_eq!(scene.click(&[PointerId::Mouse]), []);

    scene.move_to(PointerId::Mouse, 500.0);
    let log = scene.click(&[PointerId::Mouse]);
    assert_eq!(scene.focus(), None);
    assert_eq!(log, [("lost", covered)]);
}

/// Two pointers that both resolve: the one whose UI the user sees on top
/// decides — a 3D-transformed layer before the window's layout, the window
/// before a `<surface>` in the world behind it.
#[test]
fn pointers_are_asked_in_visual_order() {
    let mut scene = Scene::new();
    let on_window = scene.node(0.0, field());
    let in_layer = scene.node(200.0, field());
    let in_surface = scene.node(400.0, field());
    let remapped = scene.remapped(250.0);
    let surface = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(0x5eed));
    scene.pointer(surface, 450.0);
    scene
        .app
        .world_mut()
        .get_resource_or_init::<crate::ext::VirtualPointers>()
        .register(surface);
    scene.pointer(PointerId::Mouse, 50.0);
    scene.settle();

    scene.click(&[surface, PointerId::Mouse]);
    assert_eq!(scene.focus(), Some(on_window));
    scene.click(&[surface, PointerId::Mouse, remapped]);
    assert_eq!(scene.focus(), Some(in_layer));

    // The mouse over nothing focusable: the surface's pointer decides.
    scene.move_to(PointerId::Mouse, 700.0);
    scene.click(&[PointerId::Mouse, surface]);
    assert_eq!(scene.focus(), Some(in_surface));
}
