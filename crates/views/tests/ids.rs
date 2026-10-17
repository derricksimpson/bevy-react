//! Instance slots are reused, and an id names its instance and no other.

use std::sync::Arc;

use bevy_react_views::{
    Action, Binding, Block, Compiled, Dispatch, Effect, Key, ListItem, Runtime, TNode, Target,
    Template, Value, e,
};

fn row() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None).on_do("click", vec![Action::set(1, e::lit("clicked"))]),
            TNode::new("text", Some(0)),
        ],
        bindings: vec![Binding {
            target: Target::Text(1),
            expr: e::concat([e::slot(0), e::lit(":"), e::slot(1)]),
        }],
        slots: vec![Value::Num(0.0), Value::str("")],
        effects: vec![Effect::on(&[], 5).with_cleanup()],
        ..Default::default()
    })
}

fn table(windowed: bool) -> Arc<Compiled> {
    let block = Block::list(0, row());
    if windowed {
        Compiled::new(Template {
            nodes: vec![TNode::new("node", None), TNode::new("node", Some(0))],
            blocks: vec![block.windowed(1, 0, 1)],
            slots: vec![Value::Num(0.0), Value::Num(0.0)],
            ..Default::default()
        })
    } else {
        Compiled::new(Template {
            nodes: vec![TNode::new("node", None)],
            blocks: vec![block],
            ..Default::default()
        })
    }
}

fn item(id: i64) -> ListItem {
    ListItem::new(id, vec![Value::Num(id as f64), Value::str("")])
}

#[test]
fn scrolling_a_huge_list_forever_does_not_grow_the_table() {
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(true), 0);
    rt.set_list(t, 0, (0..10_000).map(item).collect()).unwrap();
    rt.set_window(t, 0, 0, 30).unwrap();
    rt.flush();
    rt.take_effects();
    let high_water = rt.instance_table_len();
    assert_eq!(high_water, 1 + 30);
    for step in 1..2000usize {
        let start = (step * 37) % 9_900;
        rt.set_window(t, 0, start, start + 30).unwrap();
        rt.flush();
        rt.take_effects();
    }
    assert_eq!(rt.live_instances(), 31);
    assert!(
        rt.instance_table_len() <= high_water + 30,
        "the table grew to {} after 2000 scrolls",
        rt.instance_table_len()
    );
}

#[test]
fn a_dropped_instances_id_names_nothing_even_after_its_slot_is_reused() {
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(false), 0);
    rt.set_list(t, 0, (1..=3).map(item).collect()).unwrap();
    rt.flush();
    let old = rt.list_item(t, 0, &Key::Int(2)).unwrap();
    let old_button = rt.node_id(old, 0).unwrap();
    assert_eq!(rt.get(old, 0), Some(&Value::Num(2.0)));

    // Row 2 goes, row 9 arrives and takes its slot.
    rt.set_list(t, 0, vec![ListItem::same(1), item(9), ListItem::same(3)])
        .unwrap();
    rt.flush();
    let new = rt.list_item(t, 0, &Key::Int(9)).unwrap();
    assert_ne!(old, new, "same slot, different generation");

    // The old id reaches nothing: reads, writes and its old node's events.
    assert_eq!(rt.get(old, 0), None);
    rt.set(old, 1, Value::str("ghost"));
    assert_eq!(
        rt.flush(),
        vec![],
        "a write to a dropped instance is ignored"
    );
    assert_eq!(
        rt.get(new, 1),
        Some(&Value::str("")),
        "the new row is untouched"
    );
    assert_eq!(rt.dispatch(old_button, "click"), Dispatch::None);
    assert_eq!(rt.node_id(old, 0), None);
    assert!(rt.update_item(old, 0, &Key::Int(1), vec![]).is_err());
}

#[test]
fn unmount_then_remount_keeps_the_table_small_and_ids_distinct() {
    let mut rt = Runtime::with_id_base(1);
    let mut seen = Vec::new();
    for _ in 0..500 {
        let id = rt.mount(&table(false), 0);
        assert!(!seen.contains(&id), "an id was handed out twice");
        seen.push(id);
        rt.unmount(id);
        rt.flush();
    }
    assert_eq!(rt.instance_table_len(), 1, "one slot, reused");
    assert_eq!(rt.live_instances(), 0);
}

#[test]
fn pending_effects_of_a_reused_slot_are_not_confused_with_the_old_ones() {
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(false), 0);
    rt.set_list(t, 0, vec![item(1)]).unwrap();
    rt.flush();
    let calls = rt.take_effects();
    let first = calls[0].instance;
    // Row 1 leaves and row 2 arrives into the same slot.
    rt.set_list(t, 0, vec![item(2)]).unwrap();
    rt.flush();
    let calls = rt.take_effects();
    let cleanup = calls
        .iter()
        .find(|c| c.instance == first)
        .expect("row 1's cleanup");
    assert_eq!(cleanup.js, 5);
    let second = calls
        .iter()
        .find(|c| c.instance != first)
        .expect("row 2's run");
    assert_ne!(second.instance, first);
}
