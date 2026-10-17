//! Keyed lists: the stress table's rows as a compiled `For` block,
//!
//! ```tsx
//! <node>{rows.map(r => <button key={r.id} onClick={…}><text>{`${r.id} ${r.label}`}</text></button>)}</node>
//! ```
//!
//! Every test applies the command stream to a model of the Bevy hierarchy
//! ([`Tree`]) and checks the rows it ends with, so a wrong anchor or a lost
//! move fails even when the op count looks right.

use std::collections::HashMap;
use std::sync::Arc;

use bevy_react_views::{
    Binding, Block, Command, Compiled, Key, ListError, ListItem, NodeId, Runtime, TNode, Target,
    Template, Value, e,
};

const ROOT: NodeId = 0;
const SELECT: u32 = 3;

fn row() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None).on("click", SELECT),
            TNode::new("text", Some(0)),
        ],
        bindings: vec![Binding {
            target: Target::Text(1),
            expr: e::concat([e::slot(0), e::lit(" "), e::slot(1)]),
        }],
        // Fields: 0 = id, 1 = label.
        slots: vec![Value::Num(0.0), Value::str("")],
        ..Default::default()
    })
}

fn table() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row())],
        ..Default::default()
    })
}

fn item(id: i64) -> ListItem {
    ListItem::new(
        id,
        vec![Value::Num(id as f64), Value::str(&format!("row{id}"))],
    )
}

/// A model of the bridge's hierarchy: what `apply_js_ops` would build.
#[derive(Default)]
struct Tree {
    children: HashMap<NodeId, Vec<NodeId>>,
    text: HashMap<NodeId, String>,
    alive: HashMap<NodeId, &'static str>,
}

impl Tree {
    fn apply(&mut self, cmds: &[Command]) {
        for c in cmds {
            match c {
                Command::Create { id, kind, text, .. } => {
                    assert!(self.alive.insert(*id, kind).is_none(), "{id} created twice");
                    if let Some(t) = text {
                        self.text.insert(*id, t.to_string());
                    }
                }
                Command::Append { parent, child } => {
                    self.detach(*child);
                    self.children.entry(*parent).or_default().push(*child);
                }
                Command::Insert {
                    parent,
                    child,
                    before,
                } => {
                    self.detach(*child);
                    let kids = self.children.entry(*parent).or_default();
                    let at = kids
                        .iter()
                        .position(|k| k == before)
                        .expect("anchor is a child");
                    kids.insert(at, *child);
                }
                Command::Remove { parent, child } => {
                    let kids = self.children.get_mut(parent).expect("parent has children");
                    kids.retain(|k| k != child);
                    self.despawn(*child);
                }
                Command::UpdateText { id, text } => {
                    assert!(self.alive.contains_key(id), "text of a dead node {id}");
                    self.text.insert(*id, text.to_string());
                }
                Command::SetProp { .. } | Command::SetStyle { .. } => {}
            }
        }
    }

    fn detach(&mut self, child: NodeId) {
        for kids in self.children.values_mut() {
            kids.retain(|k| *k != child);
        }
    }

    fn despawn(&mut self, id: NodeId) {
        self.alive.remove(&id);
        self.text.remove(&id);
        for k in self.children.remove(&id).unwrap_or_default() {
            self.despawn(k);
        }
    }

    /// The labels of the table's rows, in order.
    fn rows(&self, table: NodeId) -> Vec<String> {
        self.children
            .get(&table)
            .map(|rows| {
                rows.iter()
                    .map(|r| self.text[&self.children[r][0]].clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn labels(ids: &[i64]) -> Vec<String> {
    ids.iter().map(|id| format!("{id} row{id}")).collect()
}

struct Fixture {
    rt: Runtime,
    tree: Tree,
    table: usize,
    node: NodeId,
}

impl Fixture {
    fn new(ids: &[i64]) -> Self {
        let mut rt = Runtime::with_id_base(1);
        let table = rt.mount(&table(), ROOT);
        rt.set_list(table, 0, ids.iter().map(|&i| item(i)).collect())
            .unwrap();
        let mut tree = Tree::default();
        tree.apply(&rt.flush());
        let node = rt.node_id(table, 0).unwrap();
        Fixture {
            rt,
            tree,
            table,
            node,
        }
    }

    /// Apply `items`, flush, and return the commands.
    fn set(&mut self, items: Vec<ListItem>) -> Vec<Command> {
        self.rt.set_list(self.table, 0, items).unwrap();
        let cmds = self.rt.flush();
        self.tree.apply(&cmds);
        cmds
    }

    fn rows(&self) -> Vec<String> {
        self.tree.rows(self.node)
    }
}

/// Unchanged records, as the JS identity diff sends them.
fn same(ids: &[i64]) -> Vec<ListItem> {
    ids.iter().map(|&i| ListItem::same(i)).collect()
}

fn count(cmds: &[Command], pred: fn(&Command) -> bool) -> usize {
    cmds.iter().filter(|c| pred(c)).count()
}

/// React's own child reconciliation (`reconcileChildrenArray`'s
/// `lastPlacedIndex` rule): an item moves when its old index is below the
/// highest old index placed so far.
fn react_moves(old: &[i64], new: &[i64]) -> usize {
    let at: HashMap<i64, usize> = old.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut last = 0;
    let mut moves = 0;
    for k in new {
        if let Some(&o) = at.get(k) {
            if o < last {
                moves += 1;
            } else {
                last = o;
            }
        }
    }
    moves
}

#[test]
fn first_list_is_created_and_appended_in_order() {
    let f = Fixture::new(&[1, 2, 3]);
    assert_eq!(f.rows(), labels(&[1, 2, 3]));
}

#[test]
fn an_unchanged_list_sends_nothing() {
    let mut f = Fixture::new(&[1, 2, 3]);
    assert_eq!(f.set(same(&[1, 2, 3])), vec![]);
}

#[test]
fn swapping_two_rows_of_a_thousand_is_two_moves() {
    let old: Vec<i64> = (0..1000).collect();
    let mut new = old.clone();
    new.swap(1, 998);
    let mut f = Fixture::new(&old);
    let cmds = f.set(same(&new));
    assert_eq!(f.rows(), labels(&new));
    assert_eq!(cmds.len(), 2);
    assert!(cmds.iter().all(|c| matches!(c, Command::Insert { .. })));
    // The same edit, React's way: every row between the two swapped ones moves.
    assert_eq!(react_moves(&old, &new), 997);
}

#[test]
fn removing_every_second_row_only_removes() {
    let old: Vec<i64> = (0..10).collect();
    let new: Vec<i64> = old.iter().copied().filter(|i| i % 2 == 0).collect();
    let mut f = Fixture::new(&old);
    let cmds = f.set(same(&new));
    assert_eq!(f.rows(), labels(&new));
    assert_eq!(cmds.len(), 5);
    assert!(cmds.iter().all(|c| matches!(c, Command::Remove { .. })));
    assert_eq!(
        f.rt.live_instances(),
        1 + 5,
        "removed rows' instances are dropped"
    );
}

#[test]
fn appending_creates_and_appends_in_order() {
    let mut f = Fixture::new(&[1, 2]);
    let mut items = same(&[1, 2]);
    items.extend([item(3), item(4)]);
    let cmds = f.set(items);
    assert_eq!(f.rows(), labels(&[1, 2, 3, 4]));
    assert_eq!(count(&cmds, |c| matches!(c, Command::Insert { .. })), 0);
    assert_eq!(
        count(
            &cmds,
            |c| matches!(c, Command::Append { parent, .. } if *parent != 0)
        ),
        4
    );
}

#[test]
fn inserting_in_the_middle_is_one_insert() {
    let mut f = Fixture::new(&[1, 2, 3, 4]);
    let cmds = f.set(vec![
        ListItem::same(1),
        ListItem::same(2),
        item(9),
        ListItem::same(3),
        ListItem::same(4),
    ]);
    assert_eq!(f.rows(), labels(&[1, 2, 9, 3, 4]));
    assert_eq!(count(&cmds, |c| matches!(c, Command::Insert { .. })), 1);
}

#[test]
fn a_changed_record_updates_only_its_text() {
    let mut f = Fixture::new(&[1, 2, 3]);
    let mut items = same(&[1, 2, 3]);
    items[1] = ListItem::new(2, vec![Value::Num(2.0), Value::str("row2 !!!")]);
    let cmds = f.set(items);
    assert_eq!(cmds.len(), 1);
    assert!(matches!(&cmds[0], Command::UpdateText { text, .. } if &**text == "2 row2 !!!"));
    // A record re-sent with equal fields (a new object, same values) is silent.
    assert_eq!(
        f.set(vec![item(1), ListItem::same(2), ListItem::same(3)]),
        vec![]
    );
}

#[test]
fn reversing_keeps_one_row_in_place() {
    let old: Vec<i64> = (0..8).collect();
    let new: Vec<i64> = old.iter().rev().copied().collect();
    let mut f = Fixture::new(&old);
    let cmds = f.set(same(&new));
    assert_eq!(f.rows(), labels(&new));
    assert_eq!(cmds.len(), 7);
}

#[test]
fn clearing_removes_every_row_and_drops_their_instances() {
    let mut f = Fixture::new(&[1, 2, 3]);
    let cmds = f.set(vec![]);
    assert_eq!(cmds.len(), 3);
    assert_eq!(f.rows(), Vec::<String>::new());
    assert_eq!(f.rt.live_instances(), 1);
}

#[test]
fn bad_lists_are_refused_and_change_nothing() {
    let mut f = Fixture::new(&[1, 2]);
    let t = f.table;
    assert_eq!(
        f.rt.set_list(t, 0, vec![ListItem::same(1), ListItem::same(1)])
            .unwrap_err(),
        ListError::DuplicateKey(Key::Int(1))
    );
    assert_eq!(
        f.rt.set_list(t, 0, vec![ListItem::same(7)]).unwrap_err(),
        ListError::MissingFields(Key::Int(7))
    );
    assert_eq!(
        f.rt.set_list(t, 0, vec![ListItem::new(7, vec![])])
            .unwrap_err(),
        ListError::WrongFieldCount {
            key: Key::Int(7),
            expected: 2,
            got: 0
        }
    );
    assert_eq!(
        f.rt.set_list(t, 3, vec![]).unwrap_err(),
        ListError::NoSuchList
    );
    assert_eq!(f.rt.flush(), vec![]);
    assert_eq!(f.rows(), labels(&[1, 2]));
}

#[test]
fn a_row_click_finds_its_row() {
    let f = Fixture::new(&[10, 20]);
    let row20 = f.rt.list_item(f.table, 0, &Key::Int(20)).unwrap();
    let button = f.rt.node_id(row20, 0).unwrap();
    let (js, instance) = f.rt.handler(button, "click").unwrap();
    assert_eq!(js, SELECT);
    assert_eq!(
        f.rt.get(instance, 0),
        Some(&Value::Num(20.0)),
        "the handler can read its row's id"
    );
}

#[test]
fn unmounting_the_table_drops_every_row() {
    let mut f = Fixture::new(&[1, 2, 3]);
    let row1 = f.rt.list_item(f.table, 0, &Key::Int(1)).unwrap();
    let button = f.rt.node_id(row1, 0).unwrap();
    f.rt.unmount(f.table);
    assert_eq!(
        f.rt.flush(),
        vec![Command::Remove {
            parent: ROOT,
            child: f.node
        }]
    );
    assert_eq!(f.rt.live_instances(), 0);
    assert_eq!(f.rt.handler(button, "click"), None);
}

/// Random edits (removes, inserts, shuffles, label changes), each checked
/// against the model, with the move count held to the minimum.
#[test]
fn random_edits_always_land_on_the_new_list() {
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut rand = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let mut ids: Vec<i64> = (0..20).collect();
    let mut next_id = 100;
    let mut f = Fixture::new(&ids);
    for round in 0..500 {
        let old = ids.clone();
        for _ in 0..rand(4) {
            if !ids.is_empty() {
                ids.remove(rand(ids.len()));
            }
        }
        for _ in 0..rand(4) {
            ids.insert(rand(ids.len() + 1), next_id);
            next_id += 1;
        }
        for _ in 0..rand(3) {
            if ids.len() > 1 {
                let (a, b) = (rand(ids.len()), rand(ids.len()));
                ids.swap(a, b);
            }
        }
        let items: Vec<ListItem> = ids
            .iter()
            .map(|&i| {
                if old.contains(&i) {
                    ListItem::same(i)
                } else {
                    item(i)
                }
            })
            .collect();
        let cmds = f.set(items);
        assert_eq!(f.rows(), labels(&ids), "round {round}");
        assert_eq!(f.rt.live_instances(), 1 + ids.len(), "round {round}");
        let kept: Vec<i64> = ids.iter().copied().filter(|i| old.contains(i)).collect();
        let moves = count(&cmds, |c| {
            matches!(
                c,
                Command::Insert { .. } | Command::Append { parent: 1, .. }
            )
        }) - (ids.len() - kept.len());
        assert_eq!(
            moves,
            kept.len() - lis_len(&old, &kept),
            "round {round}: moves are minimal"
        );
    }
}

/// Length of the longest run of `kept` that is in `old`'s order.
fn lis_len(old: &[i64], kept: &[i64]) -> usize {
    let at: HashMap<i64, usize> = old.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut tails: Vec<usize> = Vec::new();
    for k in kept {
        let v = at[k];
        let p = tails.partition_point(|&t| t < v);
        if p == tails.len() {
            tails.push(v);
        } else {
            tails[p] = v;
        }
    }
    tails.len()
}
