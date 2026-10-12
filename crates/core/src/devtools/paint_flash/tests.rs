use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;
use bevy::render::sync_world::MainEntity;
use bevy::time::TimeUpdateStrategy;
use bevy::ui_render::ExtractedUiNodes;

use super::*;
use crate::devtools::DevtoolsConfig;
use crate::devtools::test_util::test_app;
use crate::protocol::NodeId;

/// The devtools app with a bridge, the panel open, paint flashing on, and
/// 100 ms of time per update.
fn flash_app() -> App {
    let (mut app, _rx) = test_app(DevtoolsConfig {
        settings_path: None,
        ..default()
    });
    let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel();
    std::mem::forget(out_rx);
    let (ops_tx, ops_rx) = crossbeam_channel::unbounded();
    std::mem::forget(ops_tx);
    let root = app.world_mut().spawn_empty().id();
    app.insert_resource(JsBridge::new(ops_rx, out_tx, root));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        100,
    )));
    let mut state = app.world_mut().resource_mut::<DevtoolsState>();
    state.open = true;
    state.paint_flash = true;
    // The first update only advances the clock (its delta is zero).
    app.update();
    app
}

/// A React node: 100×40 px with its top-left corner at (100, 50).
fn react_node(app: &mut App, id: NodeId) -> Entity {
    let node = app
        .world_mut()
        .spawn((
            ReactNode(id),
            ComputedNode {
                size: Vec2::new(100.0, 40.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_xy(150.0, 70.0),
        ))
        .id();
    app.world_mut()
        .resource_mut::<JsBridge>()
        .nodes
        .insert(id, node);
    node
}

fn repaint(app: &App, nodes: &[Entity]) {
    app.world()
        .resource::<RepaintFeed>()
        .push(nodes.iter().copied());
}

/// Every flash box: its entity, whether it is shown, its rect and alpha.
fn boxes(app: &mut App) -> Vec<(Entity, bool, Rect, f32)> {
    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, &Node, &BackgroundColor), With<PaintFlashBox>>();
    query
        .iter(app.world())
        .map(|(e, node, color)| {
            let px = |val| match val {
                Val::Px(px) => px,
                other => panic!("a px rect, got {other:?}"),
            };
            let min = Vec2::new(px(node.left), px(node.top));
            let size = Vec2::new(px(node.width), px(node.height));
            (
                e,
                node.display != Display::None,
                Rect::from_corners(min, min + size),
                color.0.alpha(),
            )
        })
        .collect()
}

/// A repainted React node gets one box over its border box at full
/// strength; the box fades, hides, and is reused by the next repaint.
/// Entities that are not React nodes never flash.
#[test]
fn a_repainted_react_node_flashes_and_fades() {
    let mut app = flash_app();
    let node = react_node(&mut app, 1);
    let stray = app
        .world_mut()
        .spawn((ComputedNode::default(), UiGlobalTransform::default()))
        .id();

    repaint(&app, &[node, stray, node]);
    app.update();
    let shown = boxes(&mut app);
    assert_eq!(shown.len(), 1, "one box, for the React node only");
    let (flash_box, visible, rect, alpha) = shown[0];
    assert!(visible);
    assert_eq!(rect, Rect::new(100.0, 50.0, 200.0, 90.0));
    assert_eq!(alpha, PEAK_ALPHA);

    // 100 ms a frame against a 400 ms fade.
    app.update();
    assert!((boxes(&mut app)[0].3 - PEAK_ALPHA * 0.75).abs() < 1e-4);
    // A repaint mid-fade restarts it in place.
    repaint(&app, &[node]);
    app.update();
    assert_eq!(boxes(&mut app)[0].3, PEAK_ALPHA);
    for _ in 0..5 {
        app.update();
    }
    assert!(!boxes(&mut app)[0].1, "faded out: hidden, kept for reuse");

    repaint(&app, &[node]);
    app.update();
    let shown = boxes(&mut app);
    assert_eq!(shown.len(), 1, "the pooled box is reused");
    assert_eq!((shown[0].0, shown[0].1), (flash_box, true));
}

/// The panel's own nodes repaint all the time; they never flash.
#[test]
fn the_panel_never_flashes() {
    let mut app = flash_app();
    let panel = react_node(&mut app, 100);
    let row = react_node(&mut app, 101);
    app.world_mut().entity_mut(row).insert(ChildOf(panel));
    app.world_mut().resource_mut::<DevtoolsState>().panel_root = Some(100);

    repaint(&app, &[panel, row]);
    app.update();
    assert!(boxes(&mut app).is_empty());
}

/// Off (or the panel closed) the render world records nothing and the main
/// world spawns nothing; switching off mid-flash lets the boxes fade out.
#[test]
fn off_records_and_draws_nothing() {
    let mut app = flash_app();
    let node = react_node(&mut app, 1);
    repaint(&app, &[node]);
    app.update();
    assert_eq!(boxes(&mut app).len(), 1);

    app.world_mut().resource_mut::<DevtoolsState>().open = false;
    app.update();
    let feed = app.world().resource::<RepaintFeed>().clone();
    assert!(!feed.0.on.load(Ordering::Relaxed));

    // The render-world half, against bevy's own changed set.
    let mut render_world = World::new();
    render_world.insert_resource(feed.clone());
    let mut extracted = ExtractedUiNodes::default();
    extracted.changed.insert(MainEntity::from(node));
    render_world.insert_resource(extracted);
    render_world.run_system_once(record_repaints).unwrap();
    assert!(feed.take().is_empty(), "nothing recorded while off");

    for _ in 0..5 {
        app.update();
    }
    assert!(!boxes(&mut app)[0].1, "the live flash still fades out");
    assert!(app.world().resource::<PaintFlashes>().live.is_empty());

    // Back on: the same system hands the set over.
    app.world_mut().resource_mut::<DevtoolsState>().open = true;
    app.update();
    render_world.run_system_once(record_repaints).unwrap();
    assert_eq!(feed.take(), vec![node]);
}
