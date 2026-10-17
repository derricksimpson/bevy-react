//! Row selection, the stress table's last piece:
//!
//! ```tsx
//! function Table() {
//!   const [selected, setSelected] = useState<number | null>(null);
//!   return <node>{rows.map(r => <Row key={r.id} row={r} isSelected={r.id === selected} />)}</node>;
//! }
//! // Row: style={{ backgroundColor: isSelected ? SEL : ROW }}
//! ```
//!
//! `isSelected` is a selector flag: a selection change flips only the rows
//! that gain or lose it, however long the list.

use std::sync::Arc;

use bevy_react_views::{
    Binding, Block, Command, Compiled, Key, ListItem, Runtime, TNode, Target, Template, Value, e,
};

const SELECTED: u16 = 0;
const SEL: &str = "#89b4fa";
const ROW: &str = "#45475a";

fn row() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("button", None), TNode::new("text", Some(0))],
        bindings: vec![
            Binding {
                target: Target::Text(1),
                expr: e::concat([e::slot(0), e::lit(" "), e::slot(1)]),
            },
            Binding {
                target: Target::Style(0, "backgroundColor"),
                expr: e::cond(e::slot(2), e::lit(SEL), e::lit(ROW)),
            },
        ],
        // 0 = id, 1 = label, 2 = isSelected (the selector's flag).
        slots: vec![Value::Num(0.0), Value::str(""), Value::Bool(false)],
        ..Default::default()
    })
}

fn table() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row()).select(SELECTED, 0, 2)],
        slots: vec![Value::Null],
        ..Default::default()
    })
}

fn item(id: i64) -> ListItem {
    ListItem::new(
        id,
        vec![Value::Num(id as f64), Value::str(&format!("r{id}"))],
    )
}

fn same(ids: &[i64]) -> Vec<ListItem> {
    ids.iter().map(|&i| ListItem::same(i)).collect()
}

fn recolor(rt: &Runtime, t: usize, id: i64, color: &str) -> Command {
    let r = rt.list_item(t, 0, &Key::Int(id)).unwrap();
    Command::SetStyle {
        id: rt.node_id(r, 0).unwrap(),
        key: "backgroundColor",
        value: Value::str(color),
    }
}

fn setup(n: i64) -> (Runtime, usize) {
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(), 0);
    rt.set_list(t, 0, (1..=n).map(item).collect()).unwrap();
    rt.flush();
    (rt, t)
}

fn bg(cmds: &[Command], id: u32) -> Option<Value> {
    cmds.iter().find_map(|c| match c {
        Command::Create { id: i, style, .. } if *i == id => style
            .iter()
            .find(|(k, _)| *k == "backgroundColor")
            .map(|(_, v)| v.clone()),
        _ => None,
    })
}

#[test]
fn nothing_is_selected_at_first() {
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(), 0);
    rt.set_list(t, 0, vec![item(1), item(2)]).unwrap();
    let cmds = rt.flush();
    for id in [1, 2] {
        let r = rt.list_item(t, 0, &Key::Int(id)).unwrap();
        assert_eq!(bg(&cmds, rt.node_id(r, 0).unwrap()), Some(Value::str(ROW)));
    }
}

#[test]
fn selecting_flips_one_row() {
    let (mut rt, t) = setup(1000);
    rt.set(t, SELECTED, Value::Num(500.0));
    assert_eq!(rt.flush(), vec![recolor(&rt, t, 500, SEL)]);
}

#[test]
fn moving_the_selection_flips_the_old_and_the_new_row() {
    let (mut rt, t) = setup(1000);
    rt.set(t, SELECTED, Value::Num(2.0));
    rt.flush();
    rt.set(t, SELECTED, Value::Num(900.0));
    assert_eq!(
        rt.flush(),
        vec![recolor(&rt, t, 2, ROW), recolor(&rt, t, 900, SEL)]
    );
}

#[test]
fn clearing_and_selecting_a_missing_row() {
    let (mut rt, t) = setup(10);
    rt.set(t, SELECTED, Value::Num(3.0));
    rt.flush();
    rt.set(t, SELECTED, Value::Num(99.0)); // no such row
    assert_eq!(rt.flush(), vec![recolor(&rt, t, 3, ROW)]);
    rt.set(t, SELECTED, Value::Num(4.0));
    rt.flush();
    rt.set(t, SELECTED, Value::Null);
    assert_eq!(rt.flush(), vec![recolor(&rt, t, 4, ROW)]);
}

#[test]
fn a_selection_changed_back_inside_a_window_sends_nothing() {
    let (mut rt, t) = setup(10);
    rt.set(t, SELECTED, Value::Num(3.0));
    rt.set(t, SELECTED, Value::Null);
    assert_eq!(rt.flush(), vec![]);
}

#[test]
fn a_row_created_under_a_selection_is_born_selected() {
    let (mut rt, t) = setup(3);
    rt.set(t, SELECTED, Value::Num(7.0)); // not mounted yet
    rt.flush();
    let mut items = same(&[1, 2, 3]);
    items.push(item(7));
    rt.set_list(t, 0, items).unwrap();
    let cmds = rt.flush();
    let r7 = rt.list_item(t, 0, &Key::Int(7)).unwrap();
    assert_eq!(
        bg(&cmds, rt.node_id(r7, 0).unwrap()),
        Some(Value::str(SEL)),
        "created with the selected color, no later recolor"
    );
    assert!(cmds.iter().all(|c| !matches!(c, Command::SetStyle { .. })));
}

#[test]
fn the_selected_row_can_leave_and_come_back() {
    let (mut rt, t) = setup(3);
    rt.set(t, SELECTED, Value::Num(2.0));
    rt.flush();
    rt.set_list(t, 0, same(&[1, 3])).unwrap();
    rt.flush();
    rt.set(t, SELECTED, Value::Num(3.0)); // old selection (2) has no row
    assert_eq!(rt.flush(), vec![recolor(&rt, t, 3, SEL)]);
    rt.set_list(t, 0, vec![ListItem::same(1), item(2), ListItem::same(3)])
        .unwrap();
    let cmds = rt.flush();
    let r2 = rt.list_item(t, 0, &Key::Int(2)).unwrap();
    assert_eq!(bg(&cmds, rt.node_id(r2, 0).unwrap()), Some(Value::str(ROW)));
}

#[test]
fn string_keys_select_too() {
    let row = Compiled::new(Template {
        nodes: vec![TNode::new("button", None)],
        bindings: vec![Binding {
            target: Target::Style(0, "backgroundColor"),
            expr: e::cond(e::slot(1), e::lit(SEL), e::lit(ROW)),
        }],
        slots: vec![Value::Null, Value::Bool(false)],
        ..Default::default()
    });
    let table = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row).select(0, 0, 1)],
        slots: vec![Value::Null],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table, 0);
    rt.set_list(
        t,
        0,
        vec![
            ListItem::new("a", vec![Value::str("a")]),
            ListItem::new("b", vec![Value::str("b")]),
        ],
    )
    .unwrap();
    rt.flush();
    rt.set(t, 0, Value::str("b"));
    let cmds = rt.flush();
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], Command::SetStyle { value, .. } if *value == Value::str(SEL)));
}

#[test]
#[should_panic(expected = "flag slots are the item's last slots")]
fn a_flag_slot_that_is_not_last_is_rejected() {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row()).select(SELECTED, 0, 1)],
        slots: vec![Value::Null],
        ..Default::default()
    });
}
