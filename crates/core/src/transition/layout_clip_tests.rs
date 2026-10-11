//! `overflow` clips under `transition: { layout }`: bevy_ui clips in the
//! clipping node's local space, through its whole `UiGlobalTransform`, so the
//! FLIP scale the layout channel composes there carries the clip along — a
//! clipping box reveals and hides at its EASED size, in drawing and in
//! picking (both test `CalculatedClip`), outside and inside a promoted layer.
//!
//! Harness: real layout ([`layout_app`]) with bevy's clipping after the
//! layout transition driver, as in the plugin's schedule.

use super::layout::drive_layout_transitions;
use super::layout_tests::{global, layout_app, layout_spec, root_row, step};
use super::*;
use bevy::ui::update::update_clipping_system;
use bevy::ui::{CalculatedClip, Overflow};

fn clip_app() -> App {
    let mut app = layout_app();
    app.add_systems(
        PostUpdate,
        update_clipping_system.after(drive_layout_transitions),
    );
    app
}

fn sized(width: f32) -> Node {
    Node {
        width: Val::Px(width),
        height: Val::Px(100.0),
        flex_shrink: 0.0,
        ..Default::default()
    }
}

/// A 200 px wide clipping box with a `layout` transition (one second,
/// linear) holding a 200 px wide child that never shrinks, at the row's
/// start: `(wrapper, child)`, laid out.
fn reveal_scene(app: &mut App, parent: Entity, wrapper: Node) -> (Entity, Entity) {
    let wrapper = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::clip(),
                ..wrapper
            },
            ChildOf(parent),
            layout_spec(1.0),
            TransitionState::default(),
        ))
        .id();
    let child = app.world_mut().spawn((sized(200.0), ChildOf(wrapper))).id();
    step(app, 0.016);
    (wrapper, child)
}

fn set_width(app: &mut App, e: Entity, width: f32) {
    app.world_mut()
        .entity_mut(e)
        .get_mut::<Node>()
        .unwrap()
        .width = Val::Px(width);
}

fn shows(app: &App, e: Entity, x: f32) -> bool {
    app.world()
        .get::<CalculatedClip>(e)
        .expect("clipped")
        .contains_point(Vec2::new(x, 50.0))
}

/// The box narrows 200 → 100: its children are clipped at the width it
/// SHOWS — 200 on the change frame, 150 halfway, 100 settled.
#[test]
fn overflow_clip_follows_the_eased_box() {
    let mut app = clip_app();
    let root = app.world_mut().spawn(root_row()).id();
    let (wrapper, child) = reveal_scene(&mut app, root, sized(200.0));
    set_width(&mut app, wrapper, 100.0);
    step(&mut app, 0.0);
    assert_eq!(
        global(&app, wrapper).matrix2.x_axis.x,
        2.0,
        "the FLIP scale"
    );
    assert!(shows(&app, child, 150.0) && !shows(&app, child, 210.0));
    step(&mut app, 0.5);
    assert!(shows(&app, child, 140.0) && !shows(&app, child, 160.0));
    step(&mut app, 10.0);
    assert!(shows(&app, child, 90.0) && !shows(&app, child, 110.0));
}

/// The anchor rule the guide documents: children ride the box's CENTER. So
/// content pinned to the start edge jumps by half the size change on the
/// change frame (then eases back), while content centered in the box stays on
/// the shown box's center the whole way — no jump, both ends cropped equally.
#[test]
fn content_rides_the_boxes_center() {
    let mut app = clip_app();
    let root = app.world_mut().spawn(root_row()).id();
    let (wrapper, child) = reveal_scene(&mut app, root, sized(200.0));
    assert_eq!(global(&app, child).translation.x, 100.0);
    set_width(&mut app, wrapper, 100.0);
    step(&mut app, 0.0);
    assert_eq!(global(&app, child).translation.x, 150.0, "start-pinned");
    step(&mut app, 10.0);
    assert_eq!(global(&app, child).translation.x, 100.0);

    let mut app = clip_app();
    let root = app.world_mut().spawn(root_row()).id();
    let centered = Node {
        justify_content: JustifyContent::Center,
        ..sized(200.0)
    };
    let (wrapper, child) = reveal_scene(&mut app, root, centered);
    assert_eq!(global(&app, child).translation.x, 100.0);
    set_width(&mut app, wrapper, 100.0);
    for (secs, center) in [(0.0, 100.0), (0.5, 75.0), (10.0, 50.0)] {
        step(&mut app, secs);
        assert_eq!(global(&app, wrapper).translation.x, center);
        assert_eq!(global(&app, child).translation.x, center, "centered");
    }
}

/// The same reveal inside a promoted layer (itself under a scrollport): the
/// capture clips the child at the eased box — the scrollport above the root
/// stripped — and the layer re-captures every flight frame, then caches.
#[test]
fn clip_reveal_follows_the_flip_scale_inside_a_layer() {
    use crate::layer::clip::tests::interior_clip;
    use crate::layer::clip::{LayerClips, clip_aabb, sync_layer_clips};
    use crate::layer::{
        LayerContentDirt, LayerGroupAlpha, LayerMembership, LayerRepaintState, LayersRegistry,
        PromotedLayer, PromotionReasons, resolve_layer_repaints, sync_layer_geometry,
    };
    let mut app = clip_app();
    app.init_resource::<LayerMembership>();
    app.init_resource::<LayersRegistry>();
    app.init_resource::<LayerRepaintState>();
    app.init_resource::<LayerContentDirt>();
    app.init_resource::<LayerClips>();
    app.add_systems(
        PostUpdate,
        (
            sync_layer_geometry,
            sync_layer_clips,
            resolve_layer_repaints,
        )
            .chain()
            .after(update_clipping_system),
    );
    let port = app
        .world_mut()
        .spawn(Node {
            overflow: Overflow::clip(),
            ..root_row()
        })
        .id();
    let layer = app
        .world_mut()
        .spawn((
            sized(400.0),
            ChildOf(port),
            crate::bridge::ReactNode(1),
            LayerGroupAlpha(1.0),
            PromotedLayer {
                reasons: PromotionReasons(PromotionReasons::FILTER),
            },
        ))
        .id();
    let (wrapper, child) = reveal_scene(&mut app, layer, sized(200.0));
    step(&mut app, 0.016);
    let dirty = |app: &App| {
        app.world()
            .resource::<LayerRepaintState>()
            .dirty
            .contains(&layer)
    };
    assert!(!dirty(&app), "settled");

    set_width(&mut app, wrapper, 100.0);
    for (secs, width) in [(0.0, 200.0), (0.5, 150.0), (10.0, 100.0)] {
        step(&mut app, secs);
        let clip = interior_clip(app.world_mut(), child).expect("the box clips");
        assert_eq!(clip.rects().map(<[_]>::len), Some(1), "the box alone");
        let bounds = clip_aabb(&clip).unwrap();
        assert!(
            bounds.min.x.abs() < 1e-3 && (bounds.max.x - width).abs() < 1e-3,
            "after {secs}s: {bounds:?}"
        );
        assert!(dirty(&app), "re-captured after {secs}s");
    }
    step(&mut app, 0.016);
    assert!(!dirty(&app), "cached again");
}
