//! Windowed (virtual) lists: a table of 10k rows that mounts only the rows
//! in view, between two spacers sized from the unmounted counts.
//!
//! ```tsx
//! <node>
//!   <node style={{ height: top * ROW }} />
//!   {rows.slice(start, end).map(r => <Row key={r.id} … />)}
//!   <node style={{ height: bottom * ROW }} />
//! </node>
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use bevy_react_views::{
    Binding, Block, Command, Compiled, Key, ListError, ListItem, NodeId, Runtime, TNode, Target,
    Template, Value, e, visible_range,
};

mod common;
use common::Tree;

const SELECTED: u16 = 0;
const TOP: u16 = 1;
const BOTTOM: u16 = 2;
const ROW_H: f64 = 20.0;

/// Row slots: 0 = id, 1 = label, 2 = isSelected (the selector's flag).
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
                expr: e::cond(e::slot(2), e::lit("sel"), e::lit("row")),
            },
        ],
        slots: vec![Value::Num(0.0), Value::str(""), Value::Bool(false)],
        ..Default::default()
    })
}

fn table() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("node", Some(0)),
            TNode::new("node", Some(0)),
        ],
        bindings: vec![
            Binding {
                target: Target::Style(1, "height"),
                expr: e::mul(e::slot(TOP), e::lit(ROW_H)),
            },
            Binding {
                target: Target::Style(2, "height"),
                expr: e::mul(e::slot(BOTTOM), e::lit(ROW_H)),
            },
        ],
        blocks: vec![
            Block::list(0, row())
                .select(SELECTED, 0, 2)
                .windowed(2, TOP, BOTTOM),
        ],
        slots: vec![Value::Null, Value::Num(0.0), Value::Num(0.0)],
        ..Default::default()
    })
}

fn item(id: i64) -> ListItem {
    ListItem::new(
        id,
        vec![Value::Num(id as f64), Value::str(&format!("r{id}"))],
    )
}

fn same(ids: impl IntoIterator<Item = i64>) -> Vec<ListItem> {
    ids.into_iter().map(ListItem::same).collect()
}

struct Fixture {
    rt: Runtime,
    tree: Tree,
    t: usize,
    list: NodeId,
    heights: HashMap<NodeId, f64>,
    top: NodeId,
    bottom: NodeId,
}

impl Fixture {
    /// A table of rows `0..n`, windowed to `window` from the start.
    fn new(n: i64, window: (usize, usize)) -> Self {
        let mut rt = Runtime::with_id_base(1);
        let t = rt.mount(&table(), 0);
        let (list, top, bottom) = (
            rt.node_id(t, 0).unwrap(),
            rt.node_id(t, 1).unwrap(),
            rt.node_id(t, 2).unwrap(),
        );
        rt.set_list(t, 0, (0..n).map(item).collect()).unwrap();
        rt.set_window(t, 0, window.0, window.1).unwrap();
        let mut f = Fixture {
            rt,
            tree: Tree::default(),
            t,
            list,
            heights: HashMap::new(),
            top,
            bottom,
        };
        f.flush();
        f
    }

    fn flush(&mut self) -> Vec<Command> {
        let cmds = self.rt.flush();
        for c in &cmds {
            match c {
                Command::Create { id, style, .. } => {
                    if let Some((_, Value::Num(h))) = style.iter().find(|(k, _)| *k == "height") {
                        self.heights.insert(*id, *h);
                    }
                }
                Command::SetStyle {
                    id,
                    key: "height",
                    value: Value::Num(h),
                } => {
                    self.heights.insert(*id, *h);
                }
                _ => {}
            }
        }
        self.tree.apply(&cmds);
        cmds
    }

    /// The ids of the mounted rows, in DOM order (the spacers excluded).
    fn shown(&self) -> Vec<i64> {
        self.tree.children[&self.list]
            .iter()
            .filter(|c| **c != self.top && **c != self.bottom)
            .map(|row| {
                let label = &self.tree.text[&self.tree.children[row][0]];
                label.split(' ').next().unwrap().parse().unwrap()
            })
            .collect()
    }

    fn label(&self, id: i64) -> String {
        let row = self.rt.list_item(self.t, 0, &Key::Int(id)).unwrap();
        let text = self.rt.node_id(row, 1).unwrap();
        self.tree.text[&text].clone()
    }

    fn spacers(&self) -> (f64, f64) {
        (self.heights[&self.top], self.heights[&self.bottom])
    }

    fn scroll_to(&mut self, start: usize, end: usize) -> Vec<Command> {
        self.rt.set_window(self.t, 0, start, end).unwrap();
        self.flush()
    }
}

fn count(cmds: &[Command], pred: fn(&Command) -> bool) -> usize {
    cmds.iter().filter(|c| pred(c)).count()
}

#[test]
fn only_the_window_is_mounted() {
    let f = Fixture::new(10_000, (0, 20));
    assert_eq!(f.shown(), (0..20).collect::<Vec<_>>());
    assert_eq!(
        f.rt.live_instances(),
        1 + 20,
        "the table and 20 rows, not 10,000"
    );
    assert_eq!(f.spacers(), (0.0, 9980.0 * ROW_H));
    assert_eq!(f.rt.list_window(f.t, 0), Some((0, 20, 10_000)));
}

#[test]
fn a_list_with_no_window_set_mounts_everything() {
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(), 0);
    rt.set_list(t, 0, (0..5).map(item).collect()).unwrap();
    rt.flush();
    assert_eq!(rt.live_instances(), 6);
    assert_eq!(rt.list_window(t, 0), Some((0, 5, 5)));
}

#[test]
fn scrolling_a_little_swaps_the_edges() {
    let mut f = Fixture::new(10_000, (0, 20));
    let cmds = f.scroll_to(5, 25);
    assert_eq!(f.shown(), (5..25).collect::<Vec<_>>());
    assert_eq!(count(&cmds, |c| matches!(c, Command::Remove { .. })), 5);
    assert_eq!(
        count(&cmds, |c| matches!(
            c,
            Command::Create { kind: "button", .. }
        )),
        5
    );
    assert_eq!(f.spacers(), (5.0 * ROW_H, 9975.0 * ROW_H));
    assert_eq!(f.rt.live_instances(), 21);
    // And back.
    f.scroll_to(0, 20);
    assert_eq!(f.shown(), (0..20).collect::<Vec<_>>());
    assert_eq!(f.spacers(), (0.0, 9980.0 * ROW_H));
}

#[test]
fn a_far_jump_replaces_the_rows() {
    let mut f = Fixture::new(10_000, (0, 20));
    let cmds = f.scroll_to(5000, 5020);
    assert_eq!(f.shown(), (5000..5020).collect::<Vec<_>>());
    assert_eq!(count(&cmds, |c| matches!(c, Command::Remove { .. })), 20);
    assert_eq!(f.rt.live_instances(), 21);
    assert_eq!(f.spacers(), (5000.0 * ROW_H, 4980.0 * ROW_H));
}

#[test]
fn an_unchanged_window_sends_nothing() {
    let mut f = Fixture::new(1000, (10, 30));
    assert_eq!(f.scroll_to(10, 30), vec![]);
}

#[test]
fn two_scrolls_in_one_window_apply_the_last() {
    let mut f = Fixture::new(1000, (0, 20));
    f.rt.set_window(f.t, 0, 100, 120).unwrap();
    f.rt.set_window(f.t, 0, 300, 320).unwrap();
    f.flush();
    assert_eq!(f.shown(), (300..320).collect::<Vec<_>>());
    assert_eq!(f.rt.live_instances(), 21);
}

#[test]
fn rows_keep_their_fields_while_unmounted() {
    let mut f = Fixture::new(1000, (0, 10));
    // A mounted row edited, then scrolled away and back.
    f.rt.update_item(
        f.t,
        0,
        &Key::Int(3),
        vec![Value::Num(3.0), Value::str("edited")],
    )
    .unwrap();
    f.flush();
    f.scroll_to(500, 510);
    // Edits to rows that are not mounted: by delta and through a list.
    f.rt.update_item(
        f.t,
        0,
        &Key::Int(505),
        vec![Value::Num(505.0), Value::str("delta")],
    )
    .unwrap();
    let mut items = same(0..1000);
    items[507] = item(507);
    items[507].fields = Some(vec![Value::Num(507.0), Value::str("list")]);
    f.rt.set_list(f.t, 0, items).unwrap();
    f.flush();
    assert_eq!(f.label(505), "505 delta");
    assert_eq!(f.label(507), "507 list");
    f.scroll_to(0, 10);
    assert_eq!(
        f.label(3),
        "3 edited",
        "the edit survived leaving and coming back"
    );
    f.scroll_to(500, 510);
    assert_eq!(f.label(505), "505 delta");
}

#[test]
fn inserting_at_the_top_shifts_the_window_minimally() {
    let mut f = Fixture::new(10_000, (0, 20));
    let mut items = vec![item(-1)];
    items.extend(same(0..10_000));
    f.rt.set_list(f.t, 0, items).unwrap();
    let cmds = f.flush();
    assert_eq!(f.shown(), [vec![-1], (0..19).collect()].concat());
    assert_eq!(
        count(&cmds, |c| matches!(c, Command::Remove { .. })),
        1,
        "row 19 left the window"
    );
    assert_eq!(
        count(&cmds, |c| matches!(
            c,
            Command::Create { kind: "button", .. }
        )),
        1
    );
    assert_eq!(f.spacers(), (0.0, 9981.0 * ROW_H));
    assert_eq!(f.rt.live_instances(), 21);
}

#[test]
fn removing_unmounted_rows_costs_no_dom_work() {
    let mut f = Fixture::new(10_000, (0, 20));
    let kept: Vec<i64> = (0..10_000).filter(|i| *i != 7000).collect();
    f.rt.set_list(f.t, 0, same(kept)).unwrap();
    let cmds = f.flush();
    assert_eq!(cmds.len(), 1, "only the bottom spacer: {cmds:?}");
    assert_eq!(f.spacers(), (0.0, 9979.0 * ROW_H));
    // The window onto that region skips the removed row.
    f.scroll_to(6999, 7002);
    assert_eq!(f.shown(), [6999, 7001, 7002]);
}

#[test]
fn removing_mounted_rows_removes_them() {
    let mut f = Fixture::new(1000, (0, 10));
    let kept: Vec<i64> = (0..1000).filter(|i| *i >= 5).collect();
    f.rt.set_list(f.t, 0, same(kept)).unwrap();
    f.flush();
    // The window is still the first ten positions: rows 5..15.
    assert_eq!(f.shown(), (5..15).collect::<Vec<_>>());
}

#[test]
fn a_selection_outside_the_window_is_applied_on_scrolling_in() {
    let mut f = Fixture::new(10_000, (0, 20));
    f.rt.set(f.t, SELECTED, Value::Num(5005.0));
    assert_eq!(f.flush(), vec![], "no mounted row is affected");
    let cmds = f.scroll_to(5000, 5010);
    let row = f.rt.list_item(f.t, 0, &Key::Int(5005)).unwrap();
    let id = f.rt.node_id(row, 0).unwrap();
    let color = cmds.iter().find_map(|c| match c {
        Command::Create { id: i, style, .. } if *i == id => style
            .iter()
            .find(|(k, _)| *k == "backgroundColor")
            .map(|(_, v)| v.clone()),
        _ => None,
    });
    assert_eq!(color, Some(Value::str("sel")), "born selected");
    // Scrolling away and back keeps it selected.
    f.scroll_to(0, 20);
    let cmds = f.scroll_to(5000, 5010);
    assert!(
        cmds.iter()
            .any(|c| matches!(c, Command::Create { style, .. }
        if style.contains(&("backgroundColor", Value::str("sel")))))
    );
}

#[test]
fn windows_are_clamped() {
    let mut f = Fixture::new(100, (0, 10));
    f.scroll_to(90, 500);
    assert_eq!(f.shown(), (90..100).collect::<Vec<_>>());
    assert_eq!(f.rt.list_window(f.t, 0), Some((90, 100, 100)));
    assert_eq!(f.spacers(), (90.0 * ROW_H, 0.0));
    f.scroll_to(60, 10); // end before start: nothing
    assert_eq!(f.shown(), Vec::<i64>::new());
    assert_eq!(f.rt.live_instances(), 1);
    f.scroll_to(1000, 2000); // past the end
    assert_eq!(f.shown(), Vec::<i64>::new());
}

#[test]
fn clearing_and_refilling() {
    let mut f = Fixture::new(1000, (0, 10));
    f.rt.set_list(f.t, 0, vec![]).unwrap();
    f.flush();
    assert_eq!(f.shown(), Vec::<i64>::new());
    assert_eq!(f.spacers(), (0.0, 0.0));
    f.rt.set_list(f.t, 0, (0..50).map(item).collect()).unwrap();
    f.flush();
    assert_eq!(f.shown(), (0..10).collect::<Vec<_>>());
    assert_eq!(f.spacers(), (0.0, 40.0 * ROW_H));
}

#[test]
fn a_plain_list_refuses_a_window_and_unknown_keys_are_refused() {
    let plain = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row())],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&plain, 0);
    assert_eq!(
        rt.set_window(p, 0, 0, 5).unwrap_err(),
        ListError::NotWindowed
    );

    let mut f = Fixture::new(10, (0, 5));
    assert_eq!(
        f.rt.set_list(f.t, 0, vec![ListItem::same(99)]).unwrap_err(),
        ListError::MissingFields(Key::Int(99))
    );
    // A row only in the store is a known key.
    assert!(f.rt.set_list(f.t, 0, same(0..10)).is_ok());
}

#[test]
fn a_huge_list_still_mounts_a_window() {
    let mut f = Fixture::new(50_000, (25_000, 25_020));
    assert_eq!(f.rt.live_instances(), 21);
    assert_eq!(f.shown(), (25_000..25_020).collect::<Vec<_>>());
    f.scroll_to(49_990, 50_010);
    assert_eq!(f.shown(), (49_990..50_000).collect::<Vec<_>>());
}

#[test]
fn visible_range_follows_the_scroll_position() {
    // 400px viewport, 20px rows, 3 rows of overscan.
    assert_eq!(visible_range(0.0, 400.0, 20.0, 3, 10_000), (0, 23));
    assert_eq!(visible_range(1000.0, 400.0, 20.0, 3, 10_000), (47, 73));
    assert_eq!(
        visible_range(1010.0, 400.0, 20.0, 0, 10_000),
        (50, 71),
        "partial rows count"
    );
    assert_eq!(
        visible_range(199_900.0, 400.0, 20.0, 3, 10_000),
        (9_992, 10_000)
    );
    assert_eq!(
        visible_range(-50.0, 400.0, 20.0, 0, 10_000),
        (0, 20),
        "overscroll"
    );
    assert_eq!(visible_range(0.0, 400.0, 0.0, 3, 10_000), (0, 0));
    assert_eq!(visible_range(0.0, 400.0, 20.0, 3, 0), (0, 0));
    assert_eq!(visible_range(0.0, 400.0, 20.0, 3, 5), (0, 5));
}

#[test]
#[should_panic(expected = "is not a child of node 0")]
fn an_anchor_that_is_not_a_child_is_rejected() {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("node", Some(0)),
            TNode::new("node", Some(1)),
        ],
        slots: vec![Value::Num(0.0), Value::Num(0.0)],
        blocks: vec![Block::list(0, row()).windowed(2, 0, 1)],
        ..Default::default()
    });
}

#[test]
#[should_panic(expected = "window slot 7")]
fn a_window_slot_that_is_not_declared_is_rejected() {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None), TNode::new("node", Some(0))],
        slots: vec![Value::Num(0.0)],
        blocks: vec![Block::list(0, row()).windowed(1, 0, 7)],
        ..Default::default()
    });
}
