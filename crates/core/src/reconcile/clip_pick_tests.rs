//! Stock-Bevy pin: a node outside an ancestor's clip is neither picked nor
//! hovered. Up to Bevy 0.19 the core re-filtered every `PointerHits` against
//! `CalculatedClip` itself, because bevy_ui's clip walk stopped at the first
//! parent with visible overflow — a row scrolled out of its scroll container
//! (a plain wrapper in between) kept its hover and clicks, over whatever was
//! rendered there. bevy_ui's backend tests the inherited clip now, and
//! everything downstream (hover map, pointer events, cursor, wheel targets)
//! trusts it; this fails if a future Bevy regresses.
//!
//! Harness: real layout ([`layout_app`]), bevy's clipping system, UI picking
//! backend and hover map, and the mouse pointer on a headless primary window.

use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{Location, PointerId, PointerLocation};
use bevy::picking::{InteractionPlugin, PickingPlugin, PickingSystems};
use bevy::prelude::*;
use bevy::ui::UiStack;
use bevy::ui::picking_backend::{UiPickingSettings, ui_picking};
use bevy::window::{PrimaryWindow, WindowRef};

use crate::transition::layout_tests::layout_app;

/// Inside the clipper, over the row that shows.
pub(crate) const OVER_SHOWN: Vec2 = Vec2::new(50.0, 50.0);
/// Outside the clipper, where the clipped-away row is laid out.
pub(crate) const OVER_CLIPPED: Vec2 = Vec2::new(150.0, 50.0);

pub(crate) struct ClipScene {
    pub app: App,
    pub window: Entity,
    /// The row inside the clipper (x 0..100).
    pub shown: Entity,
    /// The row laid out past the clipper's edge (x 100..200).
    pub clipped: Entity,
}

/// Real layout ([`layout_app`]), bevy's clipping system, UI picking backend
/// and hover map, with a headless primary window (returned).
pub(crate) fn picking_app() -> (App, Entity) {
    let mut app = layout_app();
    app.add_plugins((PickingPlugin, InteractionPlugin));
    app.init_resource::<UiPickingSettings>();
    app.add_systems(PreUpdate, ui_picking.in_set(PickingSystems::Backend));
    app.add_systems(
        PostUpdate,
        bevy::ui::update::update_clipping_system.after(bevy::ui::ui_layout_system),
    );
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    (app, window)
}

/// Put the mouse pointer — and the window's cursor — at `position`.
pub(crate) fn mouse_at(world: &mut World, window: Entity, position: Vec2) {
    world
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(position));
    world.spawn((
        PointerId::Mouse,
        PointerLocation::new(Location {
            target: bevy::camera::NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).unwrap(),
            ),
            position,
        }),
    ));
}

/// A 100×100 `overflow: clip` box holding a 200 px wide wrapper (visible
/// overflow — the shape the old walk got wrong) of two 100×100 rows, with the
/// mouse at `pointer`. Call `app.update()` twice: layout and clipping settle
/// in the first frame, picking reads them in the second.
pub(crate) fn clip_scene(pointer: Vec2) -> ClipScene {
    let (mut app, window) = picking_app();
    mouse_at(app.world_mut(), window, pointer);
    let world = app.world_mut();
    let fixed = |width: f32| Node {
        width: Val::Px(width),
        height: Val::Px(100.0),
        flex_shrink: 0.0,
        ..Default::default()
    };
    // Nothing propagates visibility here; the backend skips hidden nodes.
    let shown = world
        .spawn((fixed(100.0), InheritedVisibility::VISIBLE))
        .id();
    let clipped = world
        .spawn((fixed(100.0), InheritedVisibility::VISIBLE))
        .id();
    let wrapper = world
        .spawn((fixed(200.0), InheritedVisibility::VISIBLE))
        .add_children(&[shown, clipped])
        .id();
    let clipper = world
        .spawn((
            Node {
                overflow: Overflow::clip(),
                ..fixed(100.0)
            },
            InheritedVisibility::VISIBLE,
        ))
        .add_child(wrapper)
        .id();
    // bevy's stack system is private: back to front, by hand.
    world.insert_resource(UiStack {
        uinodes: vec![clipper, wrapper, shown, clipped],
        partition: std::iter::once(0..4).collect(),
    });
    ClipScene {
        app,
        window,
        shown,
        clipped,
    }
}

fn hovered(scene: &ClipScene, entity: Entity) -> bool {
    scene
        .app
        .world()
        .resource::<HoverMap>()
        .get(&PointerId::Mouse)
        .is_some_and(|hits| hits.contains_key(&entity))
}

#[test]
fn a_node_outside_an_ancestors_clip_is_not_hovered() {
    let mut scene = clip_scene(OVER_SHOWN);
    scene.app.update();
    scene.app.update();
    assert!(hovered(&scene, scene.shown), "the harness picks at all");

    let mut scene = clip_scene(OVER_CLIPPED);
    scene.app.update();
    scene.app.update();
    assert!(
        !hovered(&scene, scene.clipped),
        "a row clipped away by its grandparent must not be hovered"
    );
}
