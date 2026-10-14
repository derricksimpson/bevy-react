//! Pick mode's last step: the panel's tree expands to the picked node and
//! asks for that row to be scrolled into view (`devtools.reveal`). Only Bevy
//! knows where the row landed, so the scroll happens here, after layout.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use crate::bridge::JsBridge;
use crate::protocol::NodeId;
use crate::react_message;
use crate::scroll::{is_scroll_container, scroll_range};

/// JS → Bevy: scroll the panel so this node of its own is in view.
#[react_message(name = "devtools.reveal")]
pub(super) struct DevtoolsRevealMessage {
    id: NodeId,
}

/// The row waiting to be laid out, and for how many frames it has.
#[derive(Resource, Default)]
pub(super) struct PendingReveal(Option<(NodeId, u8)>);

/// Frames a row may take to mount and lay out before the request is dropped.
const MAX_WAIT: u8 = 30;

pub(super) fn on_reveal_message(
    msg: On<DevtoolsRevealMessage>,
    mut pending: ResMut<PendingReveal>,
) {
    pending.0 = Some((msg.event().id, 0));
}

/// Scroll the row's nearest scroll container so the row sits in its middle,
/// unless it is in view already. The row usually mounted (or its ancestors
/// expanded) this very frame, hence after layout; the offset lands with the
/// next frame's.
pub(super) fn scroll_to_revealed(
    mut pending: ResMut<PendingReveal>,
    bridge: Option<Res<JsBridge>>,
    rows: Query<(&ComputedNode, &UiGlobalTransform)>,
    parents: Query<&ChildOf>,
    mut containers: Query<(
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &mut ScrollPosition,
    )>,
) {
    let Some((id, waited)) = pending.0 else {
        return;
    };
    let row = bridge.as_ref().and_then(|b| b.nodes.get(&id).copied());
    let laid_out = row.filter(|&e| rows.get(e).is_ok_and(|(node, _)| node.size.y > 0.0));
    let Some(row) = laid_out else {
        pending.0 = (waited < MAX_WAIT).then_some((id, waited + 1));
        return;
    };
    pending.0 = None;
    let Some(container) = parents.iter_ancestors(row).find(|&e| {
        containers
            .get(e)
            .is_ok_and(|(node, ..)| is_scroll_container(node))
    }) else {
        return;
    };
    let (row_node, row_at) = rows.get(row).expect("checked above");
    let (_, view, view_at, mut pos) = containers.get_mut(container).expect("found above");
    // Physical px; the transforms already carry the applied scroll offset.
    let view_h = view.size.y - view.scrollbar_size.y;
    let top = (row_at.translation.y - row_node.size.y / 2.0)
        - (view_at.translation.y - view.size.y / 2.0);
    let Some(by) = centering_shift(top, row_node.size.y, view_h) else {
        return;
    };
    let next = ((view.scroll_position.y + by) * view.inverse_scale_factor)
        .clamp(0.0, scroll_range(view).y);
    if pos.0.y != next {
        pos.0.y = next;
    }
}

/// How far to scroll (positive = down) to center a `height`-tall row whose
/// top sits `top` below the view's top, or `None` when it shows whole.
fn centering_shift(top: f32, height: f32, view: f32) -> Option<f32> {
    let shows = top >= 0.0 && top + height <= view;
    (!shows).then(|| top - (view - height) / 2.0)
}

/// A 100px-tall view holding 1000px of rows, at rest: row 21 (rows 20px
/// tall, its top 420px down) is scrolled to the view's middle, then a row
/// already in view leaves the offset alone.
#[test]
fn the_revealed_row_scrolls_its_container() {
    use bevy::ecs::system::RunSystemOnce;

    let mut world = World::new();
    let (out_tx, _out_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_ops_tx, ops_rx) = crossbeam_channel::unbounded();
    let root = world.spawn_empty().id();
    let mut bridge = JsBridge::new(ops_rx, out_tx, root);
    let view = world
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ComputedNode {
                size: Vec2::new(200.0, 100.0),
                content_size: Vec2::new(200.0, 1000.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from_xy(100.0, 50.0),
            ScrollPosition::default(),
        ))
        .id();
    let row = |world: &mut World, top: f32| {
        world
            .spawn((
                ChildOf(view),
                ComputedNode {
                    size: Vec2::new(200.0, 20.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
                UiGlobalTransform::from_xy(100.0, top + 10.0),
            ))
            .id()
    };
    let far = row(&mut world, 420.0);
    let near = row(&mut world, 40.0);
    bridge.nodes.insert(1, far);
    bridge.nodes.insert(2, near);
    world.insert_resource(bridge);
    let reveal = |world: &mut World, id: NodeId| {
        world.insert_resource(PendingReveal(Some((id, 0))));
        world.run_system_once(scroll_to_revealed).unwrap();
        world.entity(view).get::<ScrollPosition>().unwrap().0.y
    };

    assert_eq!(reveal(&mut world, 1), 380.0);
    world.entity_mut(view).insert(ScrollPosition::default());
    assert_eq!(reveal(&mut world, 2), 0.0);
}

#[test]
fn a_row_in_view_stays_put_and_one_outside_centers() {
    assert_eq!(centering_shift(40.0, 20.0, 200.0), None);
    // Below the view: scroll down until its middle meets the view's.
    assert_eq!(centering_shift(400.0, 20.0, 200.0), Some(310.0));
    // Above it (cut by the top edge): scroll up.
    assert_eq!(centering_shift(-10.0, 20.0, 200.0), Some(-100.0));
}
