//! Hover reporting (`pointerEnter` / `pointerLeave` / `pointerHover`) for
//! elements that declared those handlers, for every pointer — split from
//! `pointer.rs` (the drag driver), whose cross-frame state it does not share.

use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use super::events::{Modifiers, send_ui_event};
use super::picking::{Geometry, event_pos, pointer_over, sample_pointer};
use crate::bridge::{HoverState, JsBridge, PointerHandlers, ReactInteraction, ReactNode};
use crate::ext::EventLocalPos;
use crate::layer::pick3d::Transform3dPointer;

/// Emit `pointerEnter` / `pointerLeave` for nodes that declared those
/// handlers. Hover in/out is the [`ReactInteraction`]
/// `None`↔(`Hovered`|`Pressed`) boundary — the same signal that drives hover
/// *styling* ([`apply_interaction_styles`](crate::reconcile::apply_interaction_styles))
/// — so it follows `focusPolicy` with no ancestor climbing. Per-node
/// [`HoverState`] remembers which pointer is inside, so a click's
/// `Hovered`↔`Pressed` transition never re-fires enter/leave, and the leave
/// reports where that pointer is.
///
/// Then `pointerHover`: each pointer that moved sends one to every node with
/// that handler in its hover-map entry (pass-through ancestors under the
/// pointer hear it too), button held or not. Boundary events go first, like
/// the DOM's `pointerenter` before `pointermove`. The cost is the moving
/// pointers' hover stacks.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn collect_hover_events(
    bridge: Res<JsBridge>,
    // `Option`: an app without picking has no hover.
    hover_map: Option<Res<HoverMap>>,
    transform3d: Option<Res<Transform3dPointer>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    cameras: Query<&Camera>,
    // `Option` so headless harnesses without input needn't init it.
    keys: Option<Res<ButtonInput<KeyCode>>>,
    // Where each pointer was when `pointerHover` last looked (moves only,
    // DOM semantics).
    mut last: Local<HashMap<PointerId, Vec2>>,
    hovering: Query<(
        &PointerHandlers,
        &ReactNode,
        Geometry,
        // SVG shapes report x/y in user units instead (see [`event_pos`]).
        Option<&EventLocalPos>,
    )>,
    mut nodes: Query<
        (
            Entity,
            &ReactInteraction,
            &mut HoverState,
            &PointerHandlers,
            &ReactNode,
            Geometry,
            Option<&EventLocalPos>,
        ),
        Changed<ReactInteraction>,
    >,
) {
    let mods = Modifiers::read(keys.as_deref());
    let transform3d = transform3d.as_deref();
    for (entity, interaction, mut hover, handlers, rnode, geometry, user) in &mut nodes {
        let inside = *interaction != ReactInteraction::None;
        if inside == hover.0.is_some() {
            continue; // A `Hovered`↔`Pressed` change, not a boundary crossing.
        }
        // Leaving: the pointer that had entered. Entering: the one over it.
        let pointer = hover.0.unwrap_or_else(|| {
            hover_map
                .as_ref()
                .and_then(|map| pointer_over(entity, map))
                .unwrap_or(PointerId::Mouse)
        });
        hover.0 = inside.then_some(pointer);
        let kind = if inside {
            "pointerEnter"
        } else {
            "pointerLeave"
        };
        if (inside && handlers.enter) || (!inside && handlers.leave) {
            // A pointer that is gone (a lifted finger) leaves at the origin.
            let sample = sample_pointer(pointer, &pointers, transform3d);
            let pos = sample
                .as_ref()
                .and_then(|s| event_pos(user, geometry, &cameras, &s.location))
                .unwrap_or(Vec2::ZERO);
            let abs = sample.map_or(Vec2::ZERO, |s| s.client);
            send_ui_event(
                &bridge,
                rnode.0,
                kind,
                Some(pos),
                Some(abs),
                None,
                Some(mods),
            );
        }
    }

    let Some(hover_map) = hover_map else {
        return;
    };
    for (&pointer, hits) in hover_map.iter() {
        let Some(sample) = sample_pointer(pointer, &pointers, transform3d) else {
            continue;
        };
        if last.insert(pointer, sample.client) == Some(sample.client) {
            continue;
        }
        for &entity in hits.keys() {
            if let Ok((handlers, rnode, geometry, user)) = hovering.get(entity)
                && handlers.hover
            {
                let pos =
                    event_pos(user, geometry, &cameras, &sample.location).unwrap_or(Vec2::ZERO);
                send_ui_event(
                    &bridge,
                    rnode.0,
                    "pointerHover",
                    Some(pos),
                    Some(sample.client),
                    None,
                    Some(mods),
                );
            }
        }
    }
    last.retain(|pointer, _| hover_map.contains_key(pointer));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::JsBridge;
    use crate::protocol::{op::Op, outbound::Outbound, outbound::UiEvent};
    use crate::reconcile::picking::test_pointers::{hover, move_to, spawn_pointer};
    use bevy::ui::{ComputedNode, UiGlobalTransform};

    /// A minimal app with a `JsBridge` (outbound receiver kept alive) and
    /// [`collect_hover_events`] scheduled.
    fn hover_app() -> (App, tokio::sync::mpsc::UnboundedReceiver<Outbound>) {
        let mut app = App::new();
        crate::schedule_shuffle::shuffle_schedules(&mut app);
        app.add_plugins(MinimalPlugins);
        let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel::<Outbound>();
        let (_ops_tx, ops_rx) = crossbeam_channel::unbounded::<Vec<Op>>();
        std::mem::forget(_ops_tx); // Keep the ops channel open for the app's lifetime.
        let root = app.world_mut().spawn_empty().id();
        app.insert_resource(JsBridge::new(ops_rx, out_tx, root));
        app.add_systems(Update, collect_hover_events);
        (app, out_rx)
    }

    fn drain_events(out_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Outbound>) -> Vec<UiEvent> {
        std::iter::from_fn(|| out_rx.try_recv().ok())
            .map(|o| match o {
                Outbound::UiEvent { event } => event,
                other => panic!("expected a UiEvent, got {other:?}"),
            })
            .collect()
    }

    /// A 200×200 physical-px box centered at (100, 100), on a target with
    /// scale factor `scale`.
    fn boxed(scale: f32) -> (ComputedNode, UiGlobalTransform) {
        (
            ComputedNode {
                size: Vec2::splat(200.0),
                inverse_scale_factor: scale.recip(),
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::splat(100.0)),
        )
    }

    /// [`collect_hover_events`] emits `pointerEnter` on the first non-`None`
    /// interaction and `pointerLeave` on the return to `None`, and must NOT re-fire
    /// on the `Hovered`↔`Pressed` transition of a click (guarded by [`HoverState`]).
    #[test]
    fn hover_events_fire_on_boundary_only() {
        let (mut app, mut out_rx) = hover_app();

        let e = app
            .world_mut()
            .spawn((
                ReactInteraction::None,
                HoverState::default(),
                PointerHandlers {
                    enter: true,
                    leave: true,
                    ..default()
                },
                ReactNode(1),
            ))
            .id();

        let set = |app: &mut App, i: ReactInteraction| {
            *app.world_mut().get_mut::<ReactInteraction>(e).unwrap() = i;
            app.update();
        };

        app.update(); // Mount frame: still "outside" (None) → no event.
        set(&mut app, ReactInteraction::Hovered); // None → Hovered: enter.
        set(&mut app, ReactInteraction::Pressed); // Hovered → Pressed: no re-enter.
        set(&mut app, ReactInteraction::None); // Pressed → None: leave.

        let kinds: Vec<String> = drain_events(&mut out_rx)
            .into_iter()
            .map(|event| {
                assert_eq!(event.id, 1);
                event.kind
            })
            .collect();
        assert_eq!(kinds, vec!["pointerEnter", "pointerLeave"]);
    }

    /// `pointerHover` goes to the nodes in the moving pointer's hover-map
    /// entry, only on a frame that pointer moved, carrying the held modifier
    /// keys and no button.
    #[test]
    fn hover_moves_fire_while_inside_on_motion() {
        let (mut app, mut out_rx) = hover_app();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ShiftLeft);
        app.insert_resource(keys);
        let node = app
            .world_mut()
            .spawn((
                PointerHandlers {
                    hover: true,
                    ..default()
                },
                ReactNode(7),
            ))
            .id();
        // Under another pointer, which never moves after its first frame.
        let other = app
            .world_mut()
            .spawn((
                PointerHandlers {
                    hover: true,
                    ..default()
                },
                ReactNode(8),
            ))
            .id();
        let a = Vec2::new(10.0, 20.0);
        let mouse = spawn_pointer(app.world_mut(), PointerId::Mouse, a);
        spawn_pointer(app.world_mut(), PointerId::Touch(3), a);
        hover(app.world_mut(), PointerId::Touch(3), &[other]);
        hover(app.world_mut(), PointerId::Mouse, &[]);
        app.update();
        drain_events(&mut out_rx); // Each pointer's first sighting.

        let mut step = |app: &mut App, at: Vec2, inside: bool| {
            move_to(app.world_mut(), mouse, at);
            let over = [node];
            hover(
                app.world_mut(),
                PointerId::Mouse,
                if inside { &over } else { &[] },
            );
            app.update();
            drain_events(&mut out_rx)
        };

        assert!(step(&mut app, a + 1.0, false).is_empty(), "outside");
        let moved = step(&mut app, a + 2.0, true);
        assert_eq!(moved.len(), 1, "the still pointer's node hears nothing");
        let ev = &moved[0];
        assert_eq!((ev.id, ev.kind.as_str()), (7, "pointerHover"));
        assert_eq!((ev.client_x, ev.client_y), (Some(12.0), Some(22.0)));
        assert_eq!(ev.button, None, "hover moves carry no button");
        assert_eq!(
            (ev.shift_key, ev.ctrl_key, ev.alt_key, ev.meta_key),
            (Some(true), Some(false), Some(false), Some(false))
        );
        assert!(
            step(&mut app, a + 2.0, true).is_empty(),
            "a still pointer sends nothing"
        );
        assert_eq!(step(&mut app, a, true).len(), 1);
        assert!(step(&mut app, a + 5.0, false).is_empty(), "left");
    }

    /// `x`/`y` are the pointer's position within the node's box: logical
    /// pointer px are converted to the node's physical px first. On a shape,
    /// the element's own [`EventLocalPos`] wins.
    #[test]
    fn hover_positions_are_node_relative_or_the_elements_own() {
        let (mut app, mut out_rx) = hover_app();
        let handlers = PointerHandlers {
            enter: true,
            ..default()
        };
        // Scale factor 2: logical (50, 25) is physical (100, 50) — the
        // middle of the 200 px box, a quarter down.
        let pointer = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(1));
        spawn_pointer(app.world_mut(), pointer, Vec2::new(50.0, 25.0));
        let node = app
            .world_mut()
            .spawn((
                ReactInteraction::None,
                HoverState::default(),
                handlers,
                ReactNode(1),
                boxed(2.0),
            ))
            .id();
        let shape = app
            .world_mut()
            .spawn((
                ReactInteraction::None,
                HoverState::default(),
                handlers,
                ReactNode(2),
                boxed(2.0),
                EventLocalPos(Some(Vec2::new(30.0, 60.0))),
            ))
            .id();
        hover(app.world_mut(), pointer, &[node, shape]);
        app.update(); // Mount frame: still outside.
        for e in [node, shape] {
            *app.world_mut().get_mut::<ReactInteraction>(e).unwrap() = ReactInteraction::Hovered;
        }
        app.update();

        let mut events = drain_events(&mut out_rx);
        events.sort_by_key(|e| e.id);
        assert_eq!(events.len(), 2);
        assert_eq!((events[0].x, events[0].y), (Some(0.5), Some(0.25)));
        assert_eq!(
            (events[0].client_x, events[0].client_y),
            (Some(50.0), Some(25.0)),
            "client coordinates are the pointer's own, in logical px"
        );
        assert_eq!(
            (events[1].x, events[1].y),
            (Some(30.0), Some(60.0)),
            "enter coordinates ride the user-space cursor"
        );
    }

    /// The transform3d pointer sits at the remapped (layout-space) position:
    /// `x`/`y` come from it, `clientX`/`clientY` stay the real cursor's.
    #[test]
    fn a_transformed_layer_reports_the_real_cursor_as_client() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, mut out_rx) = hover_app();
        app.init_resource::<bevy::picking::pointer::PointerMap>();
        app.world_mut()
            .run_system_once(crate::layer::pick3d::init_transform3d_pointer)
            .unwrap();
        let remap = app.world().resource::<Transform3dPointer>().id;
        let remap_entity = app
            .world()
            .resource::<bevy::picking::pointer::PointerMap>()
            .get_entity(remap)
            .unwrap();
        move_to(app.world_mut(), remap_entity, Vec2::new(150.0, 100.0));
        spawn_pointer(app.world_mut(), PointerId::Mouse, Vec2::new(640.0, 400.0));
        let node = app
            .world_mut()
            .spawn((
                ReactInteraction::None,
                HoverState::default(),
                PointerHandlers {
                    enter: true,
                    leave: true,
                    ..default()
                },
                ReactNode(1),
                boxed(1.0),
            ))
            .id();
        hover(app.world_mut(), remap, &[node]);
        app.update();
        *app.world_mut().get_mut::<ReactInteraction>(node).unwrap() = ReactInteraction::Hovered;
        app.update();
        // The leave is reported at the same pointer, though the map no
        // longer names the node.
        hover(app.world_mut(), remap, &[]);
        *app.world_mut().get_mut::<ReactInteraction>(node).unwrap() = ReactInteraction::None;
        app.update();

        let events = drain_events(&mut out_rx);
        assert_eq!(
            events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
            ["pointerEnter", "pointerLeave"]
        );
        for event in events {
            assert_eq!((event.x, event.y), (Some(0.75), Some(0.5)));
            assert_eq!((event.client_x, event.client_y), (Some(640.0), Some(400.0)));
        }
    }
}
