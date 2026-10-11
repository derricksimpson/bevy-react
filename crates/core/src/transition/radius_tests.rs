//! The `borderRadius` channel's tests: the value lerp, the system ease onto
//! `Node`, and — under bevy's real layout — what a circular and an
//! elliptical value resolve to.
use super::channels::ProgressChannel;
use super::layout_tests::{layout_app, root_row, step};
use super::tests::{advance, drive_world, timing};
use super::*;
use crate::animations::{Easing, Lerp};
use crate::protocol::visual::{BorderRadiusSpec, CornerRadii};
use crate::ui_map::radii_to_border_radius;

/// The radii a wire `borderRadius` string decodes to.
fn radii(wire: &str) -> CornerRadii {
    serde_json::from_value::<BorderRadiusSpec>(serde_json::json!(wire))
        .expect("decodes")
        .radii()
}

/// `Rect` lerps per leaf: same-unit corners interpolate while a corner that
/// changes unit (or reads `auto`) snaps to its target on its own.
#[test]
fn lerp_rect_eases_per_corner_and_snaps_mixed_units() {
    use crate::protocol::units::Rect as WireRect;
    use Length::*;
    let a = WireRect {
        top: Px(0.0),
        right: Px(10.0),
        bottom: Percent(0.0),
        left: Px(4.0),
    };
    let b = WireRect {
        top: Px(10.0),
        right: Px(20.0),
        bottom: Px(50.0),
        left: Percent(50.0),
    };
    let m = a.lerp(b, 0.5);
    assert_eq!(m.top, Px(5.0));
    assert_eq!(m.right, Px(15.0));
    assert_eq!(m.bottom, Px(50.0), "unit change snaps that corner");
    assert_eq!(m.left, Percent(50.0), "unit change snaps that corner");

    // `rem` corners follow the same rule.
    let a = WireRect { top: Rem(1.0), ..a };
    let b = WireRect {
        top: Rem(3.0),
        right: Rem(1.0),
        ..b
    };
    let m = a.lerp(b, 0.5);
    assert_eq!(m.top, Rem(2.0));
    assert_eq!(m.right, Rem(1.0), "px to rem snaps that corner");
}

/// `transition: { borderRadius }` eases `Node.border_radius` toward the
/// input target and pushes content dirt on every writing frame (the radius
/// is outside the layer geometry hash), then goes silent once settled.
#[test]
fn system_eases_border_radius_and_dirties_content() {
    use crate::layer::LayerContentDirt;
    use crate::protocol::units::Rect as WireRect;
    let uniform = |px: f32| WireRect {
        top: Length::Px(px),
        right: Length::Px(px),
        bottom: Length::Px(px),
        left: Length::Px(px),
    };
    let (mut world, mut schedule) = drive_world();
    let spec = Transition {
        border_radius: Some(timing(1.0, Easing::Linear)),
        ..Default::default()
    };
    let e = world
        .spawn((
            TransitionInput {
                spec,
                border_radius: Some(uniform(8.0).into()),
                ..Default::default()
            },
            TransitionState::default(),
            Node {
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..Default::default()
            },
            UiTransform::default(),
        ))
        .id();

    // First frame seeds the resting state (8) — the live value, nothing to write.
    schedule.run(&mut world);
    let dirt = std::mem::take(&mut world.resource_mut::<LayerContentDirt>().nodes);
    assert!(!dirt.contains(&e), "seed frame writes nothing");

    // Retarget to 100: halfway through a 1s linear ease → 54.
    world
        .entity_mut(e)
        .get_mut::<TransitionInput>()
        .unwrap()
        .border_radius = Some(uniform(100.0).into());
    advance(&mut world, 0.5);
    schedule.run(&mut world);
    let r = world.entity(e).get::<Node>().unwrap().border_radius;
    assert!(
        matches!(r.top_left.x, Val::Px(v) if (v - 54.0).abs() < 1.0),
        "mid expected ~54px, got {r:?}"
    );
    assert_eq!(r.bottom_right, r.top_left, "every corner eases");
    let dirt = std::mem::take(&mut world.resource_mut::<LayerContentDirt>().nodes);
    assert!(dirt.contains(&e), "an easing frame pushes content dirt");

    advance(&mut world, 0.5);
    schedule.run(&mut world);
    let r = world.entity(e).get::<Node>().unwrap().border_radius;
    assert_eq!(r, BorderRadius::all(Val::Px(100.0)), "exact settle");
    world.resource_mut::<LayerContentDirt>().nodes.clear();

    // Settled: no write, no dirt.
    advance(&mut world, 0.1);
    schedule.run(&mut world);
    let dirt = std::mem::take(&mut world.resource_mut::<LayerContentDirt>().nodes);
    assert!(!dirt.contains(&e), "a settled radius pushes no dirt");

    // Retarget to an elliptical value: the circular 100 eases as 100 / 100,
    // so halfway to "100px / 40px" the vertical radius reads 70.
    world
        .entity_mut(e)
        .get_mut::<TransitionInput>()
        .unwrap()
        .border_radius = Some(radii("100px / 40px"));
    advance(&mut world, 0.5);
    schedule.run(&mut world);
    let r = world.entity(e).get::<Node>().unwrap().border_radius;
    assert!(
        matches!(r.top_left, CornerRadius { x: Val::Px(x), y: Val::Px(y) }
            if (x - 100.0).abs() < 0.01 && (y - 70.0).abs() < 1.0),
        "mid expected ~100 / 70, got {r:?}"
    );
    let dirt = std::mem::take(&mut world.resource_mut::<LayerContentDirt>().nodes);
    assert!(dirt.contains(&e), "an easing frame pushes content dirt");

    advance(&mut world, 0.5);
    schedule.run(&mut world);
    let r = world.entity(e).get::<Node>().unwrap().border_radius;
    let settled = CornerRadius::new(Val::Px(100.0), Val::Px(40.0));
    assert_eq!(r, BorderRadius::all(settled), "exact settle");
    world.resource_mut::<LayerContentDirt>().nodes.clear();

    advance(&mut world, 0.1);
    schedule.run(&mut world);
    let dirt = std::mem::take(&mut world.resource_mut::<LayerContentDirt>().nodes);
    assert!(!dirt.contains(&e), "a settled radius pushes no dirt");
}

/// Two elliptical values ease all eight radii, each leaf on its own: one
/// that changes unit snaps alone while its corner's other axis keeps easing.
#[test]
fn lerp_elliptical_radii_eases_both_axes_per_leaf() {
    use Length::*;
    let m = radii("0px 10px / 20px").lerp(radii("10px 30px / 40px 50%"), 0.5);
    assert_eq!((m.x.top, m.x.right), (Px(5.0), Px(20.0)));
    let y = m.y.expect("stays elliptical");
    assert_eq!(y.top, Px(30.0));
    assert_eq!(y.right, Percent(50.0), "unit change snaps that leaf alone");
}

/// A circular value facing an elliptical one reads as `r / r` (CSS's meaning
/// of a single radius), so it eases; two circular values stay circular; and
/// an ease toward a circular value settles on it exactly — no leftover
/// vertical radii, which would change what a percentage means.
#[test]
fn lerp_circular_and_elliptical_radii_meet_as_r_over_r() {
    use Length::*;
    let m = radii("8px").lerp(radii("40px / 20px"), 0.5);
    assert_eq!(m.x.top, Px(24.0));
    assert_eq!(m.y.map(|y| y.top), Some(Px(14.0)));
    assert_eq!(radii("8px").lerp(radii("16px"), 0.5), radii("12px"));

    let mut ch = ProgressChannel::<CornerRadii>::default();
    ch.init(radii("40px / 20px"));
    let spec = timing(1.0, Easing::Linear);
    let mid = ch.drive(radii("50%"), Some(&spec), 0.5);
    assert!(mid.y.is_some(), "elliptical in flight");
    assert_eq!(ch.drive(radii("50%"), Some(&spec), 1.0), radii("50%"));
}

/// What the docs promise, under bevy's real layout, on a 200×80 node:
/// without a slash `"50%"` is of the shorter side (a pill — every existing
/// app); with one each radius follows its own axis (an ellipse) and is
/// capped at half the node on that axis; and bevy's hit test follows the
/// ellipse.
#[test]
fn real_layout_resolves_circular_and_elliptical_radii() {
    let mut app = layout_app();
    let root = app.world_mut().spawn(root_row()).id();
    let node = |wire: &str| Node {
        width: Val::Px(200.0),
        height: Val::Px(80.0),
        border_radius: radii_to_border_radius(radii(wire)),
        ..Default::default()
    };
    let pill = app.world_mut().spawn((node("50%"), ChildOf(root))).id();
    let ellipse = app
        .world_mut()
        .spawn((node("50% / 50%"), ChildOf(root)))
        .id();
    let capped = app
        .world_mut()
        .spawn((node("150px / 10px"), ChildOf(root)))
        .id();
    step(&mut app, 0.0);
    let top_left = |e: Entity| {
        let computed = app.world().entity(e).get::<ComputedNode>().unwrap();
        computed.border_radius.top_left
    };
    assert_eq!(top_left(pill), Vec2::splat(40.0));
    assert_eq!(top_left(ellipse), Vec2::new(100.0, 40.0));
    assert_eq!(top_left(capped), Vec2::new(100.0, 10.0));

    let computed = app.world().entity(ellipse).get::<ComputedNode>().unwrap();
    assert_eq!(computed.size, Vec2::new(200.0, 80.0));
    let global = *app
        .world()
        .entity(ellipse)
        .get::<UiGlobalTransform>()
        .unwrap();
    let center = global.translation;
    assert!(computed.contains_point(global, center));
    let corner = center - 0.5 * computed.size + Vec2::splat(2.0);
    assert!(
        !computed.contains_point(global, corner),
        "the box's corner is outside the ellipse"
    );
}
