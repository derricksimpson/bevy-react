//! Transformed picking for `transform3d` layers.
//!
//! Layer promotion is render-side only, so a 3D-transformed subtree's nodes
//! keep their untransformed layout rects on the window camera — the stock
//! hit-test sees them where layout put them, not where the composite quad
//! draws them. This module makes picking follow the visual:
//!
//! 1. [`drive_transform3d_pointer`] inverts the cursor through the topmost
//!    transformed layer's screen homography onto the layer plane and drives a
//!    shared `PointerId::Custom` virtual pointer at the recovered
//!    *untransformed* window position — stock picking then resolves the
//!    subtree's descendants normally (the `<surface>` virtual-pointer
//!    pattern, but window-target).
//! 2. [`suppress_transformed_layer_hits`] drops the real mouse pointer's hits
//!    on members of visually-transformed layers (their layout rects are
//!    visually stale) and scopes the virtual pointer's hits to the layer it
//!    is remapping into (its window location overlaps unrelated nodes).
//!
//! That is all it takes: hover, press, drags and clicks are derived from
//! bevy_picking per pointer (`reconcile::picking`), so the members are simply
//! in the virtual pointer's hover map, and event positions come from its
//! remapped location.
//!
//! Identity-valued transforms keep everything here inert (visual == layout);
//! nested transformed-in-transformed layers invert the *nearest* transformed
//! root only — a documented v1 limitation. Click-through *under* the visual
//! quad (the real pointer hitting non-members the transformed result covers)
//! is out of scope for v1.

use bevy::ecs::message::MessageMutator;
use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::picking::backend::PointerHits;
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput, PointerLocation};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, FixedNode, UiGlobalTransform, UiStack};
use bevy::window::PrimaryWindow;

use super::transform3d::LayerTransform3dMatrix;
use super::{LayerMembership, PromotedLayer};

/// The transform3d virtual pointer's fixed id (the pattern
/// `bevy_react_surface`'s in-world pointer uses too).
pub const TRANSFORM3D_POINTER_UUID: bevy::asset::uuid::Uuid =
    bevy::asset::uuid::Uuid::from_u128(0x7D3D_D001);

/// The single virtual pointer remapping cursor input into transformed layers
/// (topmost-wins, like the one surface pointer serving every surface), plus
/// its frame-to-frame state.
#[derive(Resource)]
pub struct Transform3dPointer {
    /// The custom pointer id. Picking events carrying this id originated from
    /// a transformed-layer remap.
    pub id: PointerId,
    /// The transformed layer root the pointer is currently remapped into —
    /// the scope [`suppress_transformed_layer_hits`] retains virtual-pointer
    /// hits to. `None` = parked.
    pub over_layer: Option<Entity>,
    /// Last driven position (logical window coords).
    last_pos: Vec2,
    /// The window target last driven to (kept for the park move once the
    /// cursor — and with it the mouse pointer's live location — leaves).
    last_target: Option<bevy::camera::NormalizedRenderTarget>,
    /// Press bookkeeping (owed releases).
    buttons: crate::ext::VirtualButtons,
}

/// Spawn the virtual pointer at startup and publish its id.
pub fn init_transform3d_pointer(mut commands: Commands) {
    let id = PointerId::Custom(TRANSFORM3D_POINTER_UUID);
    // Spawning a `PointerId` auto-adds `PointerLocation`/`PointerPressState`/….
    commands.spawn(id);
    commands.insert_resource(Transform3dPointer {
        id,
        over_layer: None,
        last_pos: Vec2::ZERO,
        last_target: None,
        buttons: Default::default(),
    });
}

/// The 3×3 screen homography of a layer's composite matrix: the model maps
/// plane points `(x, y, 0, 1)` to homogeneous screen `(X, Y, ·, W)`, so only
/// the x/y/w rows of the x/y/w columns matter (z is flattened at composite).
fn screen_homography(m: &Mat4) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.w),
        Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.w),
        Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.w),
    )
}

/// Map a physical-px screen position back onto the layer plane (untransformed
/// physical px). `None` when the plane is edge-on (degenerate homography),
/// the recovered point sits at infinity, or it lies behind the eye (past the
/// vanishing line, where the quad is not visible). A backface (negative
/// determinant) still inverts — backfaces render and stay clickable.
pub fn invert_screen_to_plane(model: &Mat4, screen: Vec2) -> Option<Vec2> {
    let h = screen_homography(model);
    if h.determinant().abs() < 1e-6 {
        return None; // Edge-on: the quad projects to a line.
    }
    let p = h.inverse() * screen.extend(1.0);
    if p.z.abs() < 1e-6 {
        return None; // Point at infinity on the plane.
    }
    let local = p.truncate() / p.z;
    // Forward w at the recovered point must be positive: a screen point can
    // invert onto the plane's far side (behind the eye), which never renders.
    // w = (H · (x, y, 1)).z — the homography's third row.
    let w = h.x_axis.z * local.x + h.y_axis.z * local.y + h.z_axis.z;
    if w <= 0.0 {
        return None;
    }
    Some(local)
}

/// The layer root `entity` draws into, if any. Climbs `ChildOf` first — a
/// picked leaf may be a text span outside the membership map — and stops at
/// a `FixedNode`: its subtree left the layers above it
/// ([`LayerMembership`]), so it is on screen where layout put it. `fixed`
/// is `None` while no node is fixed.
fn layer_of(
    entity: Entity,
    membership: &LayerMembership,
    child_of: &Query<&ChildOf>,
    fixed: Option<&Query<(), With<FixedNode>>>,
) -> Option<Entity> {
    let member = crate::reconcile::climb(entity, child_of, |e| {
        membership.node_to_layer.contains_key(&e) || fixed.is_some_and(|fixed| fixed.contains(e))
    })?;
    membership.node_to_layer.get(&member).copied()
}

/// The nearest visually-transformed (non-identity matrix) layer root in
/// `entity`'s enclosing-or-self layer chain, if any.
fn transformed_root_of(
    entity: Entity,
    membership: &LayerMembership,
    matrices: &Query<&LayerTransform3dMatrix>,
    child_of: &Query<&ChildOf>,
    fixed: Option<&Query<(), With<FixedNode>>>,
) -> Option<Entity> {
    let mut root = layer_of(entity, membership, child_of, fixed)?;
    loop {
        if matrices.get(root).is_ok_and(|m| m.remaps_picking()) {
            return Some(root);
        }
        root = (*membership.enclosing.get(&root)?)?;
    }
}

/// Whether `entity`'s enclosing-or-self layer chain passes through `layer`.
fn member_of_layer(
    entity: Entity,
    layer: Entity,
    membership: &LayerMembership,
    child_of: &Query<&ChildOf>,
    fixed: Option<&Query<(), With<FixedNode>>>,
) -> bool {
    let Some(mut root) = layer_of(entity, membership, child_of, fixed) else {
        return false;
    };
    loop {
        if root == layer {
            return true;
        }
        match membership.enclosing.get(&root) {
            Some(Some(outer)) => root = *outer,
            _ => return false,
        }
    }
}

/// Schedule [`drive_transform3d_pointer`]: before bevy_picking consumes
/// pointer input, and after bevy_input updated the buttons it forwards — so
/// the forwarded press lands in the frame of the mouse's own. One click is
/// then one frame of presses, which the press-to-focus rule
/// (`reconcile::focus`) resolves together; a frame apart, the mouse's press
/// on the window would blur what the forwarded press then refocuses.
pub(crate) fn add_pointer_driver(app: &mut App) {
    app.add_systems(
        PreUpdate,
        drive_transform3d_pointer
            .after(bevy::input::InputSystems)
            .before(bevy::picking::PickingSystems::ProcessInput),
    );
}

/// Remap the window cursor into the topmost visually-transformed layer under
/// it and drive the virtual pointer there (`PointerInput` move/press/release,
/// the surface driver's contract). With no transformed layer under the
/// cursor — or no transformed layers at all — the pointer parks off-bounds
/// and releases owed presses.
///
/// Scheduled before `bevy_picking`'s input processing so the new location is
/// consumed the same frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_transform3d_pointer(
    mut state: ResMut<Transform3dPointer>,
    layers: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &LayerTransform3dMatrix,
        ),
        With<PromotedLayer>,
    >,
    ui_stack: Res<UiStack>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut input: MessageWriter<PointerInput>,
) {
    let pointer_id = state.id;
    // The real mouse pointer's live location carries exactly the window
    // target picking expects; its absence (cursor off-window) parks us.
    let mouse = pointers
        .iter()
        .find(|(id, _)| matches!(id, PointerId::Mouse))
        .and_then(|(_, loc)| loc.location().cloned());
    let scale = windows.single().map(|w| w.scale_factor()).unwrap_or(1.0);

    // A press this pointer forwarded keeps its layer until the release, past
    // the layer's edge: the drag it began there follows the cursor.
    let held = state
        .buttons
        .any_pressed()
        .then_some(state.over_layer)
        .flatten();
    let candidate = mouse.as_ref().and_then(|loc| {
        let cursor = loc.position * scale;
        if let Some(root) = held {
            let (_, _, _, matrix) = layers.get(root).ok()?;
            if !matrix.remaps_picking() {
                return None;
            }
            let local = invert_screen_to_plane(&matrix.model, cursor)?;
            return Some((root, local, loc.target.clone()));
        }
        // Topmost-first: the highest UiStack (paint-order) index wins among
        // transformed layers whose inverted cursor lands in their border box.
        let mut best: Option<(usize, Entity, Vec2)> = None;
        for (root, computed, transform, matrix) in &layers {
            if !matrix.remaps_picking() {
                continue;
            }
            let size = computed.size();
            if size.x <= 0.5 || size.y <= 0.5 {
                continue;
            }
            let Some(local) = invert_screen_to_plane(&matrix.model, cursor) else {
                continue;
            };
            let min = transform.translation - size * 0.5;
            if local.x < min.x
                || local.y < min.y
                || local.x > min.x + size.x
                || local.y > min.y + size.y
            {
                continue;
            }
            let index = ui_stack.uinodes.iter().position(|&e| e == root);
            let index = index.unwrap_or(0);
            if best.is_none_or(|(top, _, _)| index > top) {
                best = Some((index, root, local));
            }
        }
        best.map(|(_, root, local)| (root, local, loc.target.clone()))
    });

    if let Some((root, local, target)) = candidate {
        let position = local / scale; // physical → logical window coords
        let location = Location {
            target: target.clone(),
            position,
        };
        let delta = position - state.last_pos;
        // A zero-delta move carries no information — unless we just remapped
        // into a (different) layer, where the move is what retargets picking.
        if delta != Vec2::ZERO || state.over_layer != Some(root) {
            input.write(PointerInput::new(
                pointer_id,
                location.clone(),
                PointerAction::Move { delta },
            ));
        }
        state.last_pos = position;
        state.last_target = Some(target);
        state.over_layer = Some(root);

        state
            .buttons
            .forward(pointer_id, &location, &buttons, &mut input);
        return;
    }

    // Parked: release owed presses where the pointer last was and move
    // off-bounds once, so picking fires `Out` and no control sticks (the
    // surface driver's leave contract).
    if state.over_layer.is_some()
        && let Some(target) = state.last_target.clone()
    {
        let at = |position| Location {
            target: target.clone(),
            position,
        };
        let (release_at, park_at) = (at(state.last_pos), at(Vec2::splat(-1.0)));
        state
            .buttons
            .leave(pointer_id, release_at, park_at, &mut input);
        state.over_layer = None;
        state.last_pos = Vec2::splat(-1.0);
    }
}

/// Scope picking hits around the transformed-layer remap: the real mouse
/// pointer must not hit members of visually-transformed layers (their layout
/// rects are stale), and the virtual pointer must hit *only* members of the
/// layer it is remapping into (its window location overlaps whatever else
/// layout put there). Runs between the picking backends and the hover-map
/// update.
pub fn suppress_transformed_layer_hits(
    mut hits: MessageMutator<PointerHits>,
    state: Option<Res<Transform3dPointer>>,
    membership: Res<LayerMembership>,
    matrices: Query<&LayerTransform3dMatrix>,
    child_of: Query<&ChildOf>,
    fixed: Query<(), With<FixedNode>>,
) {
    let Some(state) = state else {
        return;
    };
    let fixed = (!fixed.is_empty()).then_some(&fixed);
    for hits in hits.read() {
        if hits.pointer == state.id {
            match state.over_layer {
                Some(layer) => hits.picks.retain(|(entity, _)| {
                    member_of_layer(*entity, layer, &membership, &child_of, fixed)
                }),
                // Parked off-bounds: nothing it reports is meaningful.
                None => hits.picks.clear(),
            }
        } else if matches!(hits.pointer, PointerId::Mouse) {
            hits.picks.retain(|(entity, _)| {
                transformed_root_of(*entity, &membership, &matrices, &child_of, fixed).is_none()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{transform::Transform3d, transform::Transform3dOrigin};

    fn deg(
        v: f32,
    ) -> Option<crate::protocol::animatable::Animatable<crate::protocol::units::Angle>> {
        Some(crate::protocol::animatable::Animatable::Static(
            crate::protocol::units::Angle::from_radians(v.to_radians()),
        ))
    }

    /// Static-wrap a scalar channel value.
    fn st(v: f32) -> Option<crate::protocol::animatable::Animatable<f32>> {
        Some(crate::protocol::animatable::Animatable::Static(v))
    }

    /// Static-wrap an origin axis.
    fn ax(
        l: crate::protocol::units::Length,
    ) -> crate::protocol::animatable::Animatable<crate::protocol::units::Length> {
        crate::protocol::animatable::Animatable::Static(l)
    }

    /// Forward-project plane points through a perspective matrix, invert the
    /// screen position, and recover the original plane point.
    #[test]
    fn homography_inversion_round_trips() {
        let params = Transform3d {
            perspective: st(600.0),
            rotate_y: deg(35.0),
            rotate_x: deg(-12.0),
            translate_x: st(30.0),
            scale: st(1.2),
            origin: Some(Transform3dOrigin {
                x: ax(crate::protocol::units::Length::Percent(25.0)),
                y: ax(crate::protocol::units::Length::Percent(50.0)),
            }),
            ..Default::default()
        };
        let min = Vec2::new(300.0, 200.0);
        let size = Vec2::new(240.0, 160.0);
        let m = super::super::transform3d::build_transform3d_matrix(&params, min, size, 1.0);
        for local in [
            min,
            min + size,
            min + size * 0.5,
            min + Vec2::new(10.0, 100.0),
        ] {
            let screen = m.project_point3(local.extend(0.0)).truncate();
            let back = invert_screen_to_plane(&m, screen).expect("invertible");
            assert!(back.abs_diff_eq(local, 1e-2), "{local} → {screen} → {back}");
        }
    }

    /// An edge-on plane (rotateY 90°) yields no hit; a backface (rotated past
    /// 90°) still inverts.
    #[test]
    fn edge_on_misses_backface_hits() {
        let base = Transform3d {
            origin: Some(Transform3dOrigin {
                x: ax(crate::protocol::units::Length::Percent(50.0)),
                y: ax(crate::protocol::units::Length::Percent(50.0)),
            }),
            ..Default::default()
        };
        let min = Vec2::ZERO;
        let size = Vec2::new(100.0, 100.0);

        let edge_on = Transform3d {
            rotate_y: deg(90.0),
            ..base.clone()
        };
        let m = super::super::transform3d::build_transform3d_matrix(&edge_on, min, size, 1.0);
        assert!(invert_screen_to_plane(&m, Vec2::new(50.0, 50.0)).is_none());

        let backface = Transform3d {
            rotate_y: deg(150.0),
            perspective: st(800.0),
            ..base
        };
        let m = super::super::transform3d::build_transform3d_matrix(&backface, min, size, 1.0);
        let screen = m.project_point3(Vec3::new(30.0, 40.0, 0.0)).truncate();
        let back = invert_screen_to_plane(&m, screen).expect("backface inverts");
        assert!(back.abs_diff_eq(Vec2::new(30.0, 40.0), 1e-2));
    }

    /// Suppression: the mouse loses hits on transformed-layer members (but
    /// keeps unrelated ones); the virtual pointer keeps only members of the
    /// layer it is over, and everything when parked is dropped. A fixed
    /// node under a member left the layer with its subtree — it is on
    /// screen where layout put it, so it is the mouse's.
    #[test]
    fn suppression_scopes_hits_per_pointer() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::picking::backend::HitData;

        let mut world = World::new();
        world.init_resource::<Messages<PointerHits>>();
        let camera = world.spawn_empty().id();

        let transformed_root = world
            .spawn(LayerTransform3dMatrix {
                model: Mat4::from_rotation_y(0.5),
                identity: false,
                windowed: true,
            })
            .id();
        let member = world.spawn(ChildOf(transformed_root)).id();
        let unrelated = world.spawn_empty().id();
        let fixed = world.spawn((FixedNode, ChildOf(member))).id();
        let fixed_leaf = world.spawn(ChildOf(fixed)).id();

        let mut membership = LayerMembership::default();
        membership
            .node_to_layer
            .insert(transformed_root, transformed_root);
        membership.node_to_layer.insert(member, transformed_root);
        membership.enclosing.insert(transformed_root, None);
        world.insert_resource(membership);

        let virtual_id = PointerId::Custom(TRANSFORM3D_POINTER_UUID);
        world.insert_resource(Transform3dPointer {
            id: virtual_id,
            over_layer: Some(transformed_root),
            last_pos: Vec2::ZERO,
            last_target: None,
            buttons: Default::default(),
        });

        let send = |world: &mut World, pointer: PointerId, entities: &[Entity]| {
            let picks = entities
                .iter()
                .map(|&e| (e, HitData::new(camera, 0.0, None, None)))
                .collect();
            world
                .resource_mut::<Messages<PointerHits>>()
                .write(PointerHits::new(pointer, picks, 0.5));
        };
        let survivors = |world: &mut World| -> Vec<(PointerId, Vec<Entity>)> {
            world
                .resource_mut::<Messages<PointerHits>>()
                .drain()
                .map(|h| (h.pointer, h.picks.into_iter().map(|(e, _)| e).collect()))
                .collect()
        };

        // Mouse: member dropped, unrelated and the fixed subtree kept.
        let all = [member, unrelated, fixed, fixed_leaf];
        send(&mut world, PointerId::Mouse, &all);
        // Virtual pointer over the layer: member kept, the rest dropped.
        send(&mut world, virtual_id, &all);
        world
            .run_system_once(suppress_transformed_layer_hits)
            .unwrap();
        let got = survivors(&mut world);
        assert_eq!(
            got[0],
            (PointerId::Mouse, vec![unrelated, fixed, fixed_leaf])
        );
        assert_eq!(got[1], (virtual_id, vec![member]));

        // Parked virtual pointer: everything dropped.
        world.resource_mut::<Transform3dPointer>().over_layer = None;
        send(&mut world, virtual_id, &[member, unrelated]);
        world
            .run_system_once(suppress_transformed_layer_hits)
            .unwrap();
        let got = survivors(&mut world);
        assert_eq!(got[0].1, Vec::<Entity>::new());
    }

    /// A mouse press is forwarded in the frame it happens (the driver reads
    /// the buttons after bevy_input updated them).
    #[test]
    fn a_press_is_forwarded_the_frame_it_happens() {
        use bevy::input::ButtonState;
        use bevy::input::mouse::MouseButtonInput;

        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<bevy::picking::pointer::PointerMap>()
            .init_resource::<Messages<PointerInput>>()
            .add_systems(Startup, init_transform3d_pointer);
        add_pointer_driver(&mut app);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let target = bevy::camera::NormalizedRenderTarget::Window(
            bevy::window::WindowRef::Entity(window)
                .normalize(None)
                .unwrap(),
        );
        app.world_mut().spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target,
                position: Vec2::new(150.0, 50.0),
            }),
        ));
        // A 100×100 layer at 0..100, drawn 100 px to the right.
        let root = app
            .world_mut()
            .spawn((
                PromotedLayer {
                    reasons: crate::layer::PromotionReasons(
                        crate::layer::PromotionReasons::TRANSFORM3D,
                    ),
                },
                ComputedNode {
                    size: Vec2::splat(100.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
                UiGlobalTransform::from_translation(Vec2::splat(50.0)),
                LayerTransform3dMatrix {
                    model: Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0)),
                    identity: false,
                    windowed: true,
                },
            ))
            .id();
        app.world_mut().insert_resource(UiStack {
            uinodes: vec![root],
            partition: std::iter::once(0..1).collect(),
        });
        app.update();
        app.world_mut()
            .resource_mut::<Messages<PointerInput>>()
            .clear();

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window,
        });
        app.update();
        let id = app.world().resource::<Transform3dPointer>().id;
        let pressed = app
            .world_mut()
            .resource_mut::<Messages<PointerInput>>()
            .drain()
            .any(|input| input.pointer_id == id && matches!(input.action, PointerAction::Press(_)));
        assert!(pressed);
    }

    /// A press forwarded into a layer keeps that layer until the release:
    /// the cursor leaving the quad neither parks the pointer nor releases
    /// the button — the drag it began there keeps following the cursor.
    #[test]
    fn a_held_press_keeps_its_layer_past_the_edge() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::picking::pointer::PointerMap;

        let mut world = World::new();
        world.init_resource::<PointerMap>();
        world.init_resource::<Messages<PointerInput>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        let window = world.spawn((Window::default(), PrimaryWindow)).id();
        let target = bevy::camera::NormalizedRenderTarget::Window(
            bevy::window::WindowRef::Entity(window)
                .normalize(None)
                .unwrap(),
        );
        let mouse = world
            .spawn((
                PointerId::Mouse,
                PointerLocation::new(Location {
                    target: target.clone(),
                    position: Vec2::new(150.0, 50.0),
                }),
            ))
            .id();
        world.run_system_once(init_transform3d_pointer).unwrap();
        // A 100×100 layer at 0..100, drawn 100 px to the right.
        let root = world
            .spawn((
                PromotedLayer {
                    reasons: crate::layer::PromotionReasons(
                        crate::layer::PromotionReasons::TRANSFORM3D,
                    ),
                },
                ComputedNode {
                    size: Vec2::splat(100.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
                UiGlobalTransform::from_translation(Vec2::splat(50.0)),
                LayerTransform3dMatrix {
                    model: Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0)),
                    identity: false,
                    windowed: true,
                },
            ))
            .id();
        world.insert_resource(UiStack {
            uinodes: vec![root],
            partition: std::iter::once(0..1).collect(),
        });
        let id = world.resource::<Transform3dPointer>().id;
        let step = |world: &mut World, cursor: Vec2| -> Vec<(Vec2, PointerAction)> {
            world.get_mut::<PointerLocation>(mouse).unwrap().location = Some(Location {
                target: target.clone(),
                position: cursor,
            });
            world.run_system_once(drive_transform3d_pointer).unwrap();
            world.resource_mut::<ButtonInput<MouseButton>>().clear();
            world
                .resource_mut::<Messages<PointerInput>>()
                .drain()
                .filter(|input| input.pointer_id == id)
                .map(|input| (input.location.position, input.action))
                .collect()
        };
        let over = |world: &World| world.resource::<Transform3dPointer>().over_layer;

        // Press on the quad (screen 150 → plane 50).
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        let inputs = step(&mut world, Vec2::new(150.0, 50.0));
        assert!(matches!(inputs.last(), Some((_, PointerAction::Press(_)))));
        assert_eq!(over(&world), Some(root));

        // Dragged off the quad's right edge: still remapped, nothing released.
        let inputs = step(&mut world, Vec2::new(400.0, 50.0));
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].0, Vec2::new(300.0, 50.0));
        assert!(matches!(inputs[0].1, PointerAction::Move { .. }));
        assert_eq!(over(&world), Some(root));

        // Released out there: the release is where the cursor is…
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        let inputs = step(&mut world, Vec2::new(400.0, 50.0));
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].0, Vec2::new(300.0, 50.0));
        assert!(matches!(inputs[0].1, PointerAction::Release(_)));

        // …and only then does the pointer park.
        let inputs = step(&mut world, Vec2::new(400.0, 50.0));
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].0, Vec2::splat(-1.0));
        assert_eq!(over(&world), None);
    }
}
