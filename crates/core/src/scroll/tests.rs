use super::*;
use bevy::ecs::system::RunSystemOnce;

#[test]
fn scroll_range_is_zero_when_content_fits() {
    let fits = ComputedNode {
        size: Vec2::new(100.0, 100.0),
        content_size: Vec2::new(100.0, 100.0),
        inverse_scale_factor: 1.0,
        ..default()
    };
    assert_eq!(scroll_range(&fits), Vec2::ZERO);

    let overflowing = ComputedNode {
        size: Vec2::new(100.0, 100.0),
        content_size: Vec2::new(100.0, 300.0),
        inverse_scale_factor: 1.0,
        ..default()
    };
    assert_eq!(scroll_range(&overflowing), Vec2::new(0.0, 200.0));
}

/// Drive `apply_scroll` against a 200×100 scroll container centered at (300, 200)
/// holding `content_h`px of content (so the scroll range is `[0, content_h - 100]`
/// logical), with a non-scrolling child node ("text") on top of it (also carrying a
/// `ScrollPosition`, since every `Node` does). Starts the container at `start`,
/// applies one `Line` wheel tick of `wheel`, and returns the container's resulting
/// `ScrollPosition` plus the resulting [`PointerCapture`].
fn run_with_content(
    cursor: Vec2,
    start: Vec2,
    wheel: Vec2,
    content_h: f32,
) -> (Vec2, PointerCapture) {
    let mut world = World::new();

    let mut window = Window::default();
    window.set_physical_cursor_position(Some(cursor.as_dvec2()));
    world.spawn((window, PrimaryWindow));

    let container = world
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                content_size: Vec2::new(200.0, content_h),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0)),
            ScrollPosition(start),
        ))
        .id();
    // A child "text" node fully inside the container. It is NOT a scroll container
    // (overflow visible) yet still carries a `ScrollPosition` — exactly the case
    // that broke before. It sits in front of the container in the stack.
    let child = world
        .spawn((
            Node::default(),
            ComputedNode {
                size: Vec2::new(80.0, 20.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0)),
            ScrollPosition::default(),
        ))
        .id();

    // Back-to-front: container behind, child in front (so the child is hit first
    // when iterating topmost-first, and must be skipped for not opting into scroll).
    world.insert_resource(UiStack {
        uinodes: vec![container, child],
        partition: Vec::new(),
    });
    world.insert_resource(AccumulatedMouseScroll {
        unit: MouseScrollUnit::Line,
        delta: wheel,
    });
    world.insert_resource(PointerCapture::default());

    world.run_system_once(apply_scroll).unwrap();

    let pos = world.entity(container).get::<ScrollPosition>().unwrap().0;
    let capture = *world.resource::<PointerCapture>();
    (pos, capture)
}

/// Like [`run_with_content`] with the default 300px content (scroll range `[0, 200]`).
fn run(cursor: Vec2, start: Vec2, wheel: Vec2) -> (Vec2, PointerCapture) {
    run_with_content(cursor, start, wheel, 300.0)
}

/// A 200×100 scroll container holding 300px of content, plus any extra components
/// the caller adds (e.g. `ScrollStep`, `ScrollTransitionState`). Cursor centered,
/// one `Line` wheel tick of `wheel`. Returns the container entity + world.
fn run_with(extra: impl Bundle, wheel: Vec2) -> (World, Entity) {
    let mut world = World::new();
    let mut window = Window::default();
    window.set_physical_cursor_position(Some(Vec2::new(300.0, 200.0).as_dvec2()));
    world.spawn((window, PrimaryWindow));
    let container = world
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                content_size: Vec2::new(200.0, 300.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0)),
            ScrollPosition::default(),
            extra,
        ))
        .id();
    world.insert_resource(UiStack {
        uinodes: vec![container],
        partition: Vec::new(),
    });
    world.insert_resource(AccumulatedMouseScroll {
        unit: MouseScrollUnit::Line,
        delta: wheel,
    });
    world.insert_resource(PointerCapture::default());
    world.run_system_once(apply_scroll).unwrap();
    (world, container)
}

#[test]
fn scroll_step_scales_the_wheel_delta() {
    // A custom step of 40 moves twice the default 20 per line tick (wheel down).
    let (world, container) = run_with(ScrollStep(40.0), Vec2::new(0.0, -1.0));
    let pos = world.entity(container).get::<ScrollPosition>().unwrap().0;
    assert_eq!(pos, Vec2::new(0.0, 40.0));
}

#[test]
fn wheel_feeds_target_when_scroll_transition_present() {
    // With a `ScrollTransitionState`, the wheel sets the eased *target*, leaving
    // `ScrollPosition` untouched (the drive system moves it later).
    let (world, container) = run_with(ScrollTransitionState::default(), Vec2::new(0.0, -1.0));
    assert_eq!(
        world.entity(container).get::<ScrollPosition>().unwrap().0,
        Vec2::ZERO,
        "the wheel must not move the offset directly when easing"
    );
    assert_eq!(
        world
            .entity(container)
            .get::<ScrollTransitionState>()
            .unwrap()
            .target,
        Vec2::new(0.0, 20.0),
        "the wheel accumulates into the eased target"
    );
}

#[test]
fn scrolls_container_when_cursor_is_over_its_child_text() {
    // Cursor over the child "text"; wheel down (delta.y < 0) reveals lower content
    // → offset.y increases by LINE_HEIGHT, within the [0, 200] range.
    let (pos, capture) = run(Vec2::new(300.0, 200.0), Vec2::ZERO, Vec2::new(0.0, -1.0));
    assert_eq!(pos, Vec2::new(0.0, LINE_HEIGHT));
    assert!(
        capture.wheel_captured,
        "a consumed wheel must claim the wheel channel"
    );
    assert!(
        !capture.over_ui,
        "consuming the wheel is not a hover claim — `over_ui` stays untouched \
         so world drag/press gating is unaffected"
    );
}

#[test]
fn clamps_at_the_bottom() {
    // Near the end (190) + a 20px wheel-down tick would reach 210, but the max is
    // 200 (content 300 − size 100), so it clamps instead of accumulating slack.
    let (pos, _) = run(
        Vec2::new(300.0, 200.0),
        Vec2::new(0.0, 190.0),
        Vec2::new(0.0, -1.0),
    );
    assert_eq!(pos, Vec2::new(0.0, 200.0));
}

#[test]
fn clamps_at_the_top() {
    // Near the start (10) + a 20px wheel-up tick would reach −10, but it clamps to 0.
    let (pos, _) = run(
        Vec2::new(300.0, 200.0),
        Vec2::new(0.0, 10.0),
        Vec2::new(0.0, 1.0),
    );
    assert_eq!(pos, Vec2::new(0.0, 0.0));
}

/// `<surface>` UI (on an image-target camera) is wheeled by the virtual
/// pointer sitting on that image, never by the window cursor — whose
/// window px would otherwise hit texture-px geometry at the same numbers.
#[test]
fn surface_ui_scrolls_under_its_virtual_pointer_only() {
    use bevy::camera::ImageRenderTarget;
    use bevy::picking::pointer::Location;
    use bevy::reflect::{FromReflect, structs::DynamicStruct};

    let run = |pointer_at: Option<Vec2>| {
        let mut world = World::new();
        world.init_resource::<bevy::picking::pointer::PointerMap>();
        let mut window = Window::default();
        window.set_physical_cursor_position(Some(Vec2::new(300.0, 200.0).as_dvec2()));
        world.spawn((window, PrimaryWindow));
        let image = ImageRenderTarget {
            handle: Handle::<Image>::default(),
            scale_factor: 1.0,
        };
        let camera = world
            .spawn((Camera::default(), RenderTarget::Image(image.clone())))
            .id();
        let mut target = DynamicStruct::default();
        target.insert("camera", Some(camera));
        let container = world
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ComputedNode {
                    size: Vec2::new(200.0, 100.0),
                    content_size: Vec2::new(200.0, 300.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0)),
                ScrollPosition::default(),
                ComputedUiTargetCamera::from_reflect(&target).unwrap(),
            ))
            .id();
        if let Some(position) = pointer_at {
            let id = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(7));
            let mut pointers = crate::ext::VirtualPointers::default();
            pointers.register(id);
            world.insert_resource(pointers);
            world.spawn((
                id,
                PointerLocation::new(Location {
                    target: NormalizedRenderTarget::Image(image),
                    position,
                }),
            ));
        }
        world.insert_resource(UiStack {
            uinodes: vec![container],
            partition: Vec::new(),
        });
        world.insert_resource(AccumulatedMouseScroll {
            unit: MouseScrollUnit::Line,
            delta: Vec2::new(0.0, -1.0),
        });
        world.insert_resource(PointerCapture::default());
        world.run_system_once(apply_scroll).unwrap();
        world.entity(container).get::<ScrollPosition>().unwrap().0
    };
    assert_eq!(run(None), Vec2::ZERO, "the window cursor must not reach it");
    assert_eq!(
        run(Some(Vec2::new(250.0, 180.0))),
        Vec2::new(0.0, LINE_HEIGHT)
    );
    assert_eq!(
        run(Some(Vec2::splat(-1.0))),
        Vec2::ZERO,
        "parked off-bounds"
    );
}

#[test]
fn ignores_wheel_when_cursor_is_outside_the_container() {
    // In-window but outside the container's rect (x:200..400, y:150..250).
    let (pos, capture) = run(
        Vec2::new(50.0, 50.0),
        Vec2::new(0.0, 30.0),
        Vec2::new(0.0, -1.0),
    );
    assert_eq!(pos, Vec2::new(0.0, 30.0));
    assert!(!capture.wheel_captured);
}

#[test]
fn does_not_claim_wheel_when_content_fits() {
    // A scroll container whose content fits (content 100 == size 100, so the scroll
    // range is empty) under the cursor. The wheel must NOT be consumed: position stays
    // put and the pointer is left un-owned so a world system (e.g. the orbit camera's
    // wheel-zoom behind a transparent, non-overflowing scroll pane) still receives it.
    let (pos, capture) = run_with_content(
        Vec2::new(300.0, 200.0),
        Vec2::ZERO,
        Vec2::new(0.0, -1.0),
        100.0,
    );
    assert_eq!(pos, Vec2::ZERO);
    assert!(
        !capture.wheel_captured,
        "a container with nothing to scroll must not claim the wheel"
    );
}

#[test]
fn wheel_listener_on_top_blocks_ancestor_scroll() {
    // A scroll container with a `WheelListener` child on top of it (the reported
    // bug shape: an `onWheel` box inside a scrollable page). Cursor over the child
    // → `apply_scroll` must yield, leaving the ancestor container un-scrolled. The
    // event itself is emitted separately by `collect_wheel_events`.
    let mut world = World::new();
    let mut window = Window::default();
    window.set_physical_cursor_position(Some(Vec2::new(300.0, 200.0).as_dvec2()));
    world.spawn((window, PrimaryWindow));

    let container = world
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                content_size: Vec2::new(200.0, 300.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0)),
            ScrollPosition::default(),
        ))
        .id();
    let child = world
        .spawn((
            Node::default(),
            ComputedNode {
                size: Vec2::new(80.0, 20.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0)),
            ScrollPosition::default(),
            WheelListener,
        ))
        .id();
    // Container behind, `onWheel` child in front (topmost-first hits the child).
    world.insert_resource(UiStack {
        uinodes: vec![container, child],
        partition: Vec::new(),
    });
    world.insert_resource(AccumulatedMouseScroll {
        unit: MouseScrollUnit::Line,
        delta: Vec2::new(0.0, -1.0),
    });
    world.insert_resource(PointerCapture::default());
    world.run_system_once(apply_scroll).unwrap();

    assert_eq!(
        world.entity(container).get::<ScrollPosition>().unwrap().0,
        Vec2::ZERO,
        "an onWheel node on top must block its ancestor scroll container"
    );
}

/// A 200×100 scroll container (300px of content) under a 200×100 node that
/// blocks what lies beneath it, both centered on the cursor; `nested` makes the
/// blocker the container's child, else they are unrelated (an overlay). One
/// wheel tick down; returns the container's offset.
fn wheel_under_blocker(nested: bool) -> Vec2 {
    let mut world = World::new();
    let mut window = Window::default();
    window.set_physical_cursor_position(Some(Vec2::new(300.0, 200.0).as_dvec2()));
    world.spawn((window, PrimaryWindow));
    let rect = |content_h: f32| ComputedNode {
        size: Vec2::new(200.0, 100.0),
        content_size: Vec2::new(200.0, content_h),
        inverse_scale_factor: 1.0,
        ..default()
    };
    let at = UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0));
    let container = world
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            rect(300.0),
            at,
            ScrollPosition::default(),
        ))
        .id();
    // Visible as visibility propagation would leave it (a `Node`'s required
    // `InheritedVisibility` starts hidden).
    let mut blocker = world.spawn((
        Node::default(),
        rect(100.0),
        at,
        Pickable::default(),
        InheritedVisibility::VISIBLE,
    ));
    if nested {
        blocker.insert(ChildOf(container));
    }
    let blocker = blocker.id();
    world.insert_resource(UiStack {
        uinodes: vec![container, blocker],
        partition: Vec::new(),
    });
    world.insert_resource(AccumulatedMouseScroll {
        unit: MouseScrollUnit::Line,
        delta: Vec2::new(0.0, -1.0),
    });
    world.insert_resource(PointerCapture::default());
    world.run_system_once(apply_scroll).unwrap();
    world.entity(container).get::<ScrollPosition>().unwrap().0
}

#[test]
fn a_blocking_overlay_keeps_the_wheel_from_what_it_covers() {
    assert_eq!(wheel_under_blocker(false), Vec2::ZERO);
}

#[test]
fn a_blocking_child_still_scrolls_its_container() {
    assert_eq!(wheel_under_blocker(true), Vec2::new(0.0, 20.0));
}

/// The post-layout settle: a pinned-to-bottom request (`scrollTop={BIG}`)
/// clamped against STALE geometry lands short; `settle_controlled_scroll`
/// re-clamps the raw request with fresh `ComputedNode` values and removes
/// the marker. This is the "log appends rows + pins to bottom in one
/// commit" fix.
#[test]
fn settle_reclamps_pending_request_with_fresh_geometry() {
    let mut world = World::new();
    // Stale clamp landed at 200 (content was 300); fresh layout says the
    // content grew to 500 → true max 400.
    let container = world
        .spawn((
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                content_size: Vec2::new(200.0, 500.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, 200.0)),
            PendingControlledScroll(Vec2::new(0.0, 1e9)),
        ))
        .id();
    world.run_system_once(settle_controlled_scroll).unwrap();
    assert_eq!(
        world.entity(container).get::<ScrollPosition>().unwrap().0,
        Vec2::new(0.0, 400.0),
        "the raw request must re-clamp against fresh geometry"
    );
    assert!(
        world
            .entity(container)
            .get::<PendingControlledScroll>()
            .is_none(),
        "the marker is one-shot"
    );
}

/// With a scroll transition, the settle corrects the eased *target*, not
/// the live offset; an already-correct request settles silently.
#[test]
fn settle_feeds_transition_target_and_in_range_is_silent() {
    let mut world = World::new();
    let eased = world
        .spawn((
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                content_size: Vec2::new(200.0, 500.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, 50.0)),
            ScrollTransitionState::default(),
            PendingControlledScroll(Vec2::new(0.0, 1e9)),
        ))
        .id();
    world
        .entity_mut(eased)
        .get_mut::<ScrollTransitionState>()
        .unwrap()
        .target = Vec2::new(0.0, 200.0);
    // In-range request (created node whose raw offset was fine).
    let in_range = world
        .spawn((
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                content_size: Vec2::new(200.0, 500.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, 120.0)),
            PendingControlledScroll(Vec2::new(0.0, 120.0)),
        ))
        .id();
    world.run_system_once(settle_controlled_scroll).unwrap();
    assert_eq!(
        world
            .entity(eased)
            .get::<ScrollTransitionState>()
            .unwrap()
            .target,
        Vec2::new(0.0, 400.0),
        "easing: the settle corrects the target"
    );
    assert_eq!(
        world.entity(eased).get::<ScrollPosition>().unwrap().0,
        Vec2::new(0.0, 50.0),
        "easing: the live offset is the drive system's job"
    );
    assert_eq!(
        world.entity(in_range).get::<ScrollPosition>().unwrap().0,
        Vec2::new(0.0, 120.0),
        "an in-range request settles to itself"
    );
    assert!(
        world
            .entity(in_range)
            .get::<PendingControlledScroll>()
            .is_none()
    );
}

/// Drive `collect_wheel_events` against a single 200×100 `WheelListener` node (id 7)
/// centered at (300, 200), with one wheel tick of `wheel` in `unit`. Returns the
/// emitted `wheel` `UiEvent` (if any) and the resulting [`PointerCapture`].
fn run_wheel(
    cursor: Vec2,
    wheel: Vec2,
    unit: MouseScrollUnit,
) -> (Option<UiEvent>, PointerCapture) {
    let mut world = World::new();

    let mut window = Window::default();
    window.set_physical_cursor_position(Some(cursor.as_dvec2()));
    world.spawn((window, PrimaryWindow));

    let (_ops_tx, ops_rx) = crossbeam_channel::unbounded::<Vec<crate::protocol::op::Op>>();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Outbound>();
    let root = world.spawn_empty().id();
    world.insert_resource(JsBridge::new(ops_rx, out_tx, root));

    let node = world
        .spawn((
            ReactNode(7),
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(300.0, 200.0)),
            WheelListener,
        ))
        .id();
    world.insert_resource(UiStack {
        uinodes: vec![node],
        partition: Vec::new(),
    });
    world.insert_resource(AccumulatedMouseScroll { unit, delta: wheel });
    world.insert_resource(PointerCapture::default());

    world.run_system_once(collect_wheel_events).unwrap();

    let event = out_rx.try_recv().ok().map(|o| match o {
        Outbound::UiEvent { event } => event,
        other => panic!("expected a UiEvent, got {other:?}"),
    });
    let capture = *world.resource::<PointerCapture>();
    (event, capture)
}

#[test]
fn wheel_over_listener_emits_raw_delta_and_claims() {
    // Cursor over the node; the raw delta rides through untouched (no ScrollStep
    // scaling), tagged with its unit, and the wheel is claimed for the UI.
    let (event, capture) = run_wheel(
        Vec2::new(300.0, 200.0),
        Vec2::new(3.0, -2.0),
        MouseScrollUnit::Line,
    );
    let event = event.expect("a wheel event over the listener");
    assert_eq!(event.id, 7);
    assert_eq!(event.kind, "wheel");
    assert_eq!(event.delta_x, Some(3.0));
    assert_eq!(event.delta_y, Some(-2.0));
    assert_eq!(event.delta_mode.as_deref(), Some("line"));
    // Cursor at the node's center → normalized (0.5, 0.5), absolute = logical cursor.
    assert_eq!(event.x, Some(0.5));
    assert_eq!(event.y, Some(0.5));
    assert_eq!(event.client_x, Some(300.0));
    assert_eq!(event.client_y, Some(200.0));
    assert!(
        capture.wheel_captured,
        "a handled wheel must claim the wheel channel"
    );
    assert!(
        !capture.over_ui,
        "handling the wheel is not a hover claim — `over_ui` stays untouched"
    );
}

#[test]
fn wheel_reports_pixel_unit_for_trackpads() {
    let (event, _) = run_wheel(
        Vec2::new(300.0, 200.0),
        Vec2::new(0.0, 12.0),
        MouseScrollUnit::Pixel,
    );
    assert_eq!(
        event.expect("a wheel event").delta_mode.as_deref(),
        Some("pixel")
    );
}

#[test]
fn wheel_outside_listener_emits_nothing() {
    // In-window but outside the node's rect (x:200..400, y:150..250) → no event, and
    // the wheel is left un-owned so world systems still receive it.
    let (event, capture) = run_wheel(
        Vec2::new(50.0, 50.0),
        Vec2::new(0.0, -1.0),
        MouseScrollUnit::Line,
    );
    assert!(event.is_none(), "no listener under the cursor");
    assert!(!capture.wheel_captured);
}

#[test]
fn zero_wheel_emits_nothing() {
    let (event, capture) = run_wheel(Vec2::new(300.0, 200.0), Vec2::ZERO, MouseScrollUnit::Line);
    assert!(event.is_none(), "a zero delta must not emit a wheel event");
    assert!(!capture.wheel_captured);
}
