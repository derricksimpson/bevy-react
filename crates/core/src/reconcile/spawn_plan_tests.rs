//! Spawn-time parenting (`spawn_plan`): the planned fast path must build the
//! same `Children` order as the generic append path in every batch shape.

use super::spawn_plan::SpawnPlan;
use super::test_util::{children_of, create, create_node, ent, ordering_app};
use crate::bridge::{JsBridge, ReactNode};
use crate::protocol::{NodeId, ROOT_ID, op::Op};
use bevy::prelude::*;

fn append(parent: NodeId, child: NodeId) -> Op {
    Op::Append { parent, child }
}

fn planned(app: &App, ops: &[Op]) -> usize {
    SpawnPlan::build(ops, app.world().resource::<JsBridge>()).planned()
}

fn react_nodes(app: &mut App) -> usize {
    app.world_mut()
        .query::<&ReactNode>()
        .iter(app.world())
        .count()
}

/// React's order: children are created (and appended) before their parent.
#[test]
fn children_first_batch_keeps_order_and_leaves_no_stray_entities() {
    let (mut app, tx, _root) = ordering_app();
    let before = react_nodes(&mut app);
    let ops = vec![
        create_node(2),
        create_node(3),
        create_node(4),
        create_node(1),
        append(1, 2),
        append(1, 3),
        append(1, 4),
        append(ROOT_ID, 1),
    ];
    assert_eq!(planned(&app, &ops), 4);
    tx.send(ops).unwrap();
    app.update();
    assert_eq!(
        children_of(&app, ent(&app, 1)),
        vec![ent(&app, 2), ent(&app, 3), ent(&app, 4)]
    );
    assert_eq!(
        app.world().get::<ChildOf>(ent(&app, 1)).unwrap().0,
        ent(&app, ROOT_ID)
    );
    assert_eq!(react_nodes(&mut app), before + 4);
}

/// A nested tree built bottom-up (grandchildren before children before root).
#[test]
fn nested_bottom_up_batch() {
    let (mut app, tx, _root) = ordering_app();
    tx.send(vec![
        create_node(4),
        create_node(5),
        create_node(2),
        create_node(6),
        create_node(3),
        create_node(1),
        append(2, 4),
        append(2, 5),
        append(3, 6),
        append(1, 2),
        append(1, 3),
        append(ROOT_ID, 1),
    ])
    .unwrap();
    app.update();
    let e = |i| ent(&app, i);
    assert_eq!(children_of(&app, e(1)), vec![e(2), e(3)]);
    assert_eq!(children_of(&app, e(2)), vec![e(4), e(5)]);
    assert_eq!(children_of(&app, e(3)), vec![e(6)]);
}

#[test]
fn parent_first_batch() {
    let (mut app, tx, _root) = ordering_app();
    tx.send(vec![
        create_node(1),
        create_node(2),
        create_node(3),
        append(ROOT_ID, 1),
        append(1, 2),
        append(1, 3),
    ])
    .unwrap();
    app.update();
    assert_eq!(
        children_of(&app, ent(&app, 1)),
        vec![ent(&app, 2), ent(&app, 3)]
    );
}

/// Appends out of creation order cannot ride the spawn order: that parent
/// takes the generic path, and the appended order still wins.
#[test]
fn appends_out_of_creation_order_fall_back() {
    let (mut app, tx, _root) = ordering_app();
    let ops = vec![
        create_node(2),
        create_node(3),
        create_node(4),
        create_node(1),
        append(1, 4),
        append(1, 2),
        append(1, 3),
        append(ROOT_ID, 1),
    ];
    assert_eq!(planned(&app, &ops), 1, "only the root's child is planned");
    tx.send(ops).unwrap();
    app.update();
    let e = |i| ent(&app, i);
    assert_eq!(children_of(&app, e(1)), vec![e(4), e(2), e(3)]);
}

#[test]
fn positional_insert_makes_the_parent_impure() {
    let (mut app, tx, _root) = ordering_app();
    tx.send(vec![
        create_node(1),
        create_node(2),
        create_node(3),
        create_node(4),
        append(ROOT_ID, 1),
        append(1, 2),
        append(1, 3),
        Op::Insert {
            parent: 1,
            child: 4,
            before: 2,
        },
    ])
    .unwrap();
    app.update();
    let e = |i| ent(&app, i);
    assert_eq!(children_of(&app, e(1)), vec![e(4), e(2), e(3)]);
}

/// New children land after the children the parent already has, and an
/// existing child moved in the same batch keeps the shadow order.
#[test]
fn existing_parent_gets_new_children_at_the_end() {
    let (mut app, tx, _root) = ordering_app();
    tx.send(vec![
        create_node(1),
        create_node(2),
        append(ROOT_ID, 1),
        append(1, 2),
    ])
    .unwrap();
    app.update();
    let ops = vec![create_node(3), create_node(4), append(1, 3), append(1, 4)];
    assert_eq!(planned(&app, &ops), 2);
    tx.send(ops).unwrap();
    app.update();
    let id = |app: &App, i| ent(app, i);
    assert_eq!(
        children_of(&app, id(&app, 1)),
        vec![id(&app, 2), id(&app, 3), id(&app, 4)]
    );

    tx.send(vec![create_node(5), append(1, 5), append(1, 2)])
        .unwrap();
    app.update();
    assert_eq!(
        children_of(&app, id(&app, 1)),
        vec![id(&app, 3), id(&app, 4), id(&app, 5), id(&app, 2)]
    );
}

/// A detached element (`<root>`) is never parented by the op path, planned
/// siblings or not.
#[test]
fn detached_child_stays_unparented() {
    let (mut app, tx, _root) = ordering_app();
    tx.send(vec![
        create_node(2),
        create(3, "root", serde_json::json!({})),
        create_node(4),
        create_node(1),
        append(1, 2),
        append(1, 3),
        append(1, 4),
        append(ROOT_ID, 1),
    ])
    .unwrap();
    app.update();
    let e = |i| ent(&app, i);
    assert_eq!(children_of(&app, e(1)), vec![e(2), e(4)]);
    assert!(app.world().get::<ChildOf>(e(3)).is_none());
}

/// Removing an ancestor in the batch that built it despawns the whole tree.
#[test]
fn same_batch_ancestor_removal_reaches_planned_children() {
    let (mut app, tx, _root) = ordering_app();
    tx.send(vec![
        create_node(2),
        create_node(1),
        append(1, 2),
        append(ROOT_ID, 1),
        Op::Remove {
            parent: ROOT_ID,
            child: 1,
        },
    ])
    .unwrap();
    app.update();
    let bridge = app.world().resource::<JsBridge>();
    let (one, two) = (bridge.nodes.get(&1).copied(), bridge.nodes.get(&2).copied());
    for e in one.into_iter().chain(two) {
        assert!(app.world().get_entity(e).is_err(), "{e:?} leaked");
    }
}

#[test]
fn text_nodes_are_born_attached_in_order() {
    let (mut app, tx, _root) = ordering_app();
    tx.send(vec![
        Op::CreateText {
            id: 2,
            text: "a".into(),
        },
        Op::CreateText {
            id: 3,
            text: "b".into(),
        },
        create_node(1),
        append(1, 2),
        append(1, 3),
        append(ROOT_ID, 1),
    ])
    .unwrap();
    app.update();
    let e = |i| ent(&app, i);
    assert_eq!(children_of(&app, e(1)), vec![e(2), e(3)]);
}

/// Ids too sparse for the dense tables skip planning and still build right.
#[test]
fn sparse_ids_take_the_generic_path() {
    let (mut app, tx, _root) = ordering_app();
    let ops = vec![
        create_node(5),
        create_node(9_000_000),
        create_node(7),
        append(7, 5),
        append(7, 9_000_000),
        append(ROOT_ID, 7),
    ];
    assert_eq!(planned(&app, &ops), 0);
    tx.send(ops).unwrap();
    app.update();
    let id = |app: &App, i| ent(app, i);
    assert_eq!(
        children_of(&app, id(&app, 7)),
        vec![id(&app, 5), id(&app, 9_000_000)]
    );
}
