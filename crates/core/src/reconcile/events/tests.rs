use super::*;
use crate::ReactInteraction;
use crate::protocol::op::Op;
use bevy::picking::events::Pointer;

/// [`collect_scroll_events`] reports a `"scroll"` for a `ScrollListener` node
/// whose offset diverges from the recorded one, ignores non-listener nodes, and
/// records the emitted value.
#[test]
fn collect_scroll_events_emits_for_listener_only() {
    use bevy::ecs::system::RunSystemOnce;

    let mut world = World::new();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Outbound>();
    let (_ops_tx, ops_rx) = crossbeam_channel::unbounded::<Vec<Op>>();
    let root = world.spawn_empty().id();
    world.insert_resource(JsBridge::new(ops_rx, out_tx, root));

    world.spawn((
        ScrollPosition(Vec2::new(0.0, 50.0)),
        ReactNode(1),
        ScrollListener,
    ));
    // No marker → must be ignored even though its ScrollPosition is "changed".
    world.spawn((ScrollPosition(Vec2::new(0.0, 70.0)), ReactNode(2)));

    world.run_system_once(collect_scroll_events).unwrap();

    match out_rx.try_recv().expect("a scroll event for the listener") {
        Outbound::UiEvent { event } => {
            assert_eq!(event.id, 1);
            assert_eq!(event.kind, "scroll");
            assert_eq!(event.scroll_top, Some(50.0));
            assert_eq!(event.scroll_left, Some(0.0));
        }
        other => panic!("expected a UiEvent, got {other:?}"),
    }
    assert!(
        out_rx.try_recv().is_err(),
        "the non-listener node must not emit"
    );
    assert_eq!(
        world.resource::<JsBridge>().scroll_positions.get(&1),
        Some(&Vec2::new(0.0, 50.0))
    );
}

/// A `ScrollPosition` equal to the recorded value (a controlled write-back, or
/// an unchanged offset) is NOT echoed — this is what breaks the controlled
/// component's feedback loop.
#[test]
fn collect_scroll_events_dedups_controlled_writeback() {
    use bevy::ecs::system::RunSystemOnce;

    let mut world = World::new();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Outbound>();
    let (_ops_tx, ops_rx) = crossbeam_channel::unbounded::<Vec<Op>>();
    let root = world.spawn_empty().id();
    world.insert_resource(JsBridge::new(ops_rx, out_tx, root));

    // The controlled write already recorded this exact offset.
    world
        .resource_mut::<JsBridge>()
        .scroll_positions
        .insert(1, Vec2::new(0.0, 50.0));
    world.spawn((
        ScrollPosition(Vec2::new(0.0, 50.0)),
        ReactNode(1),
        ScrollListener,
    ));

    world.run_system_once(collect_scroll_events).unwrap();

    assert!(
        out_rx.try_recv().is_err(),
        "a write-back equal to the recorded value must not echo back to React"
    );
}

/// Register a virtual pointer the way a feature does (the `<surface>`
/// crate registers its in-world pointer), returning its id.
fn register_virtual_pointer(app: &mut App) -> PointerId {
    let id = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(0x5eed));
    app.world_mut().spawn(id);
    app.world_mut()
        .get_resource_or_init::<crate::ext::VirtualPointers>()
        .register(id);
    id
}

/// A synthetic picking `PointerClick` location: the render target is
/// irrelevant to the collectors, so a default image handle stands in.
fn click_location() -> bevy::picking::pointer::Location {
    bevy::picking::pointer::Location {
        target: bevy::camera::NormalizedRenderTarget::Image(
            Handle::<bevy::image::Image>::default().into(),
        ),
        position: Vec2::ZERO,
    }
}

/// A minimal app wired for the picking-based click collectors: a `JsBridge`
/// (with its outbound receiver kept alive) + `PointerClick` messages.
fn click_app() -> (App, tokio::sync::mpsc::UnboundedReceiver<Outbound>) {
    let mut app = App::new();
    app.init_resource::<bevy::picking::pointer::PointerMap>();
    app.add_plugins(MinimalPlugins);
    let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel::<Outbound>();
    let (_ops_tx, ops_rx) = crossbeam_channel::unbounded::<Vec<Op>>();
    std::mem::forget(_ops_tx); // Keep the ops channel open for the app's lifetime.
    let root = app.world_mut().spawn_empty().id();
    app.insert_resource(JsBridge::new(ops_rx, out_tx, root));
    app.add_message::<PointerClick>();
    (app, out_rx)
}

fn drain_clicks(out_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Outbound>) -> Vec<UiEvent> {
    std::iter::from_fn(|| out_rx.try_recv().ok())
        .map(|o| match o {
            Outbound::UiEvent { event } => event,
            other => panic!("expected a UiEvent, got {other:?}"),
        })
        .collect()
}

/// [`collect_ui_events`] rides `PointerClick`: only the primary button
/// clicks (right/middle are the `onPointer*` events' job), a click on a
/// node's leaf (label) climbs to the click owner, and the
/// multi-pick fan-out (leaf + owner both hovered) dedupes to ONE event.
#[test]
fn picking_click_fires_once_primary_only() {
    let (mut app, mut out_rx) = click_app();
    app.add_systems(Update, collect_ui_events);

    let owner = app.world_mut().spawn((ReactNode(1), ClickOwner)).id();
    let leaf = app.world_mut().spawn(ChildOf(owner)).id();

    let click = |entity, button| PointerClick {
        entity,
        pointer: Pointer::new(PointerId::Mouse, click_location()),
        button,
        hit: bevy::picking::backend::HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    };
    // A right click must be ignored entirely…
    app.world_mut()
        .write_message(click(leaf, PointerButton::Secondary));
    // …while a primary gesture fans out to every hovered entity (leaf +
    // owner) and must dedupe to one click.
    app.world_mut()
        .write_message(click(leaf, PointerButton::Primary));
    app.world_mut()
        .write_message(click(owner, PointerButton::Primary));
    app.update();

    let events = drain_clicks(&mut out_rx);
    assert_eq!(
        events.len(),
        1,
        "secondary filtered out; leaf + owner primary picks dedupe to one click"
    );
    assert_eq!(events[0].id, 1);
    assert_eq!(events[0].kind, "click");
    assert_eq!(
        events[0].button, None,
        "clicks carry no button (primary implied)"
    );
}

/// Clicks do NOT bubble: when nested click owners are all under the
/// pointer (pass-through hit stack), only the topmost owner — the resolving
/// hit with the smallest `HitData.depth`, not the first message (hover-map
/// order is arbitrary) — gets the click. The same collector serves every
/// pointer: the mouse, and a feature's virtual one (a `<surface>`'s).
#[test]
fn nested_onclick_owners_click_topmost_only() {
    let (mut app, mut out_rx) = click_app();
    app.add_systems(Update, collect_ui_events);
    let surface_pointer = register_virtual_pointer(&mut app);

    let outer = app.world_mut().spawn((ReactNode(1), ClickOwner)).id();
    let inner = app
        .world_mut()
        .spawn((ReactNode(2), ClickOwner, ChildOf(outer)))
        .id();
    let leaf = app.world_mut().spawn(ChildOf(inner)).id();

    for pointer in [PointerId::Mouse, surface_pointer] {
        let click = |entity, depth| PointerClick {
            entity,
            pointer: Pointer::new(pointer, click_location()),
            button: PointerButton::Primary,
            hit: bevy::picking::backend::HitData::new(Entity::PLACEHOLDER, depth, None, None),
            duration: std::time::Duration::ZERO,
            count: 1,
        };
        // Adversarial order: the DEEPEST hit arrives first (the hover map is a
        // HashMap — message order carries no meaning). Depths mirror the
        // bevy_ui backend: topmost 0.0, +0.00001 per node beneath.
        app.world_mut().write_message(click(outer, 0.00002));
        app.world_mut().write_message(click(leaf, 0.0));
        app.world_mut().write_message(click(inner, 0.00001));
        app.update();

        let events = drain_clicks(&mut out_rx);
        assert_eq!(
            events.len(),
            1,
            "a click over nested owners must fire only the topmost owner ({pointer:?})"
        );
        assert_eq!(events[0].id, 2, "the inner (topmost) node owns the click");
    }
}

/// Hover/press styling alone must NOT own a click: a hover-styled `<text>`
/// label inside a click-owning ancestor carries a `ReactInteraction` (for the
/// Bevy-side restyle) but no handler — the click still belongs to the
/// ancestor. Regression for the old interaction-as-ownership overload,
/// which made such a label swallow its button's `onClick`.
#[test]
fn hover_styled_label_does_not_steal_click() {
    let (mut app, mut out_rx) = click_app();
    app.add_systems(Update, collect_ui_events);

    let owner = app.world_mut().spawn((ReactNode(1), ClickOwner)).id();
    // The label: interactive for styling, but not a click owner.
    let label = app
        .world_mut()
        .spawn((ReactNode(2), ReactInteraction::None, ChildOf(owner)))
        .id();

    let click = |entity, depth| PointerClick {
        entity,
        pointer: Pointer::new(PointerId::Mouse, click_location()),
        button: PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(Entity::PLACEHOLDER, depth, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    };
    // The label is the topmost hit; the owner sits just beneath.
    app.world_mut().write_message(click(label, 0.0));
    app.world_mut().write_message(click(owner, 0.00001));
    app.update();

    let events = drain_clicks(&mut out_rx);
    assert_eq!(events.len(), 1, "exactly one owner resolves");
    assert_eq!(
        events[0].id, 1,
        "the hover-styled label must climb to the click-owning ancestor"
    );
}

/// No-regression pin for handler fallthrough: a click picked on a
/// handler-less shape (no `ClickOwner`) climbs the `ChildOf` chain to
/// the `<svg>` root's own ownership marker — the root still owns the click.
#[test]
fn handlerless_shape_click_falls_to_svg_root() {
    let (mut app, mut out_rx) = click_app();
    app.add_systems(Update, collect_ui_events);

    let root = app.world_mut().spawn((ReactNode(1), ClickOwner)).id();
    // A node-less, handler-less child (an SVG shape's shape, minus the
    // feature): no `ClickOwner` of its own.
    let shape = app
        .world_mut()
        .spawn((
            ReactNode(2),
            crate::ext::ElementFlags::NODE_LESS,
            ChildOf(root),
        ))
        .id();

    app.world_mut().write_message(PointerClick {
        entity: shape,
        pointer: Pointer::new(PointerId::Mouse, click_location()),
        button: PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    app.update();

    let events = drain_clicks(&mut out_rx);
    assert_eq!(events.len(), 1, "the click resolves to exactly one owner");
    assert_eq!(
        events[0].id, 1,
        "a handler-less shape's click belongs to the <svg> root"
    );
}

/// The §"a drag owns its click" fixtures: `card` (a click owner) holds
/// `slider` (dragged by the mouse), which holds `knob` (a click owner).
/// Returns `(app, rx, card, slider, knob)`.
fn dragged_slider_in_a_card() -> (
    App,
    tokio::sync::mpsc::UnboundedReceiver<Outbound>,
    [Entity; 3],
) {
    let (mut app, out_rx) = click_app();
    app.add_systems(Update, collect_ui_events);
    let card = app.world_mut().spawn((ReactNode(1), ClickOwner)).id();
    let slider = app
        .world_mut()
        .spawn((ReactNode(2), ClickOwner, ChildOf(card)))
        .id();
    let knob = app
        .world_mut()
        .spawn((ReactNode(3), ClickOwner, ChildOf(slider)))
        .id();
    app.world_mut()
        .get_resource_or_init::<crate::reconcile::ActiveDrag>()
        .begin(
            slider,
            PointerId::Mouse,
            PointerButton::Primary,
            Vec2::ZERO,
            Vec2::ZERO,
        );
    (app, out_rx, [card, slider, knob])
}

fn primary_click(entity: Entity, pointer: PointerId, depth: f32) -> PointerClick {
    PointerClick {
        entity,
        pointer: Pointer::new(pointer, click_location()),
        button: PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(Entity::PLACEHOLDER, depth, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    }
}

/// The bug: a slider dragged and released over the card around it. The
/// press landed on both (pass-through), the release hovers the card, so
/// bevy_picking clicks the card — which the drag's gesture does not own.
#[test]
fn drag_release_over_an_ancestor_does_not_click_it() {
    let (mut app, mut out_rx, [card, ..]) = dragged_slider_in_a_card();
    app.world_mut()
        .write_message(primary_click(card, PointerId::Mouse, 0.0));
    app.update();
    assert!(drain_clicks(&mut out_rx).is_empty());
}

/// The dragged element's own click stands, and so does a descendant's: a
/// `<button>` inside a press surface is clicked through the surface's drag.
#[test]
fn a_button_inside_the_dragged_surface_still_clicks() {
    let (mut app, mut out_rx, [card, slider, knob]) = dragged_slider_in_a_card();
    for (entity, depth) in [(card, 0.00002), (slider, 0.00001), (knob, 0.0)] {
        app.world_mut()
            .write_message(primary_click(entity, PointerId::Mouse, depth));
    }
    app.update();
    let events = drain_clicks(&mut out_rx);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].id, 3,
        "the topmost owner inside the dragged element"
    );
}

/// What the press hit *above* the dragged element is not beneath it: a
/// clickable badge laid over a draggable card (a sibling, not a child) keeps
/// its click, as before the rule.
#[test]
fn a_click_owner_pressed_above_the_dragged_element_still_clicks() {
    let (mut app, mut out_rx, [card, slider, _]) = dragged_slider_in_a_card();
    let badge = app
        .world_mut()
        .spawn((ReactNode(4), ClickOwner, ChildOf(card)))
        .id();
    let mut active = app
        .world_mut()
        .resource_mut::<crate::reconcile::ActiveDrag>();
    active
        .begin(
            slider,
            PointerId::Touch(9),
            PointerButton::Primary,
            Vec2::ZERO,
            Vec2::ZERO,
        )
        .above = vec![badge];
    for (entity, depth) in [(card, 0.00002), (slider, 0.00001), (badge, 0.0)] {
        app.world_mut()
            .write_message(primary_click(entity, PointerId::Touch(9), depth));
    }
    app.update();
    let events = drain_clicks(&mut out_rx);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 4);
}

/// A drag scopes its own device only: another pointer (a second finger, a
/// `<surface>`'s pointer) clicks what it likes meanwhile.
#[test]
fn another_pointers_click_is_not_scoped() {
    let (mut app, mut out_rx, [card, ..]) = dragged_slider_in_a_card();
    app.world_mut()
        .write_message(primary_click(card, PointerId::Touch(1), 0.0));
    app.update();
    let events = drain_clicks(&mut out_rx);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 1);
}

/// The mouse and its transform3d remap are one device: a drag the
/// remapped pointer runs (a slider on a tilted card) scopes the raw
/// mouse's click too.
#[test]
fn a_mouse_click_is_scoped_by_a_transform3d_drag() {
    use bevy::ecs::system::RunSystemOnce;
    let (mut app, mut out_rx) = click_app();
    app.add_systems(Update, collect_ui_events);
    app.world_mut()
        .run_system_once(crate::layer::pick3d::init_transform3d_pointer)
        .unwrap();
    let remap = app
        .world()
        .resource::<crate::layer::pick3d::Transform3dPointer>()
        .id;
    let page = app.world_mut().spawn((ReactNode(1), ClickOwner)).id();
    let slider = app.world_mut().spawn((ReactNode(2), ClickOwner)).id();
    app.world_mut()
        .get_resource_or_init::<crate::reconcile::ActiveDrag>()
        .begin(
            slider,
            remap,
            PointerButton::Primary,
            Vec2::ZERO,
            Vec2::ZERO,
        );
    app.world_mut()
        .write_message(primary_click(page, PointerId::Mouse, 0.0));
    app.world_mut()
        .write_message(primary_click(slider, remap, 0.0));
    app.update();
    let events = drain_clicks(&mut out_rx);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, 2);
}
