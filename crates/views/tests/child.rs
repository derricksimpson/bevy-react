//! Nested and conditional components:
//!
//! ```tsx
//! function Panel() {
//!   const [open, setOpen] = useState(false);
//!   const [name, setName] = useState("world");
//!   return (
//!     <node>
//!       <text>Panel</text>
//!       {open && <Detail greeting={`Hello ${name}`} />}
//!       <text>footer</text>
//!     </node>
//!   );
//! }
//! function Detail({ greeting }) { return <node><text>{greeting}</text></node>; }
//! ```
//!
//! Streams are replayed on the hierarchy model, so placement before the
//! `footer` is checked, not just the command count.

use std::sync::Arc;

use bevy_react_views::{
    Binding, Block, ChildBlock, Command, Compiled, InstanceId, ListItem, NodeId, Runtime, TNode,
    Target, Template, Value, e,
};

mod common;
use common::Tree;

const OPEN: u16 = 0;
const NAME: u16 = 1;
// Panel node indices.
const FOOTER: usize = 2;

/// `<node><text>{greeting}</text></node>`, slot 0 = greeting.
fn detail() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None), TNode::new("text", Some(0))],
        bindings: vec![Binding {
            target: Target::Text(1),
            expr: e::slot(0),
        }],
        slots: vec![Value::str("?")],
        ..Default::default()
    })
}

fn panel(detail: Arc<Compiled>) -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("text", Some(0)).text("Panel"),
            TNode::new("text", Some(0)).text("footer"),
        ],
        children: vec![
            ChildBlock::new(0, detail)
                .before(FOOTER as u16)
                .when(e::slot(OPEN))
                .prop(0, e::concat([e::lit("Hello "), e::slot(NAME)])),
        ],
        slots: vec![Value::Bool(false), Value::str("world")],
        ..Default::default()
    })
}

struct Fixture {
    rt: Runtime,
    tree: Tree,
    panel: InstanceId,
    root: NodeId,
}

impl Fixture {
    fn new() -> Self {
        let mut rt = Runtime::with_id_base(1);
        let panel = rt.mount(&panel(detail()), 0);
        let mut tree = Tree::default();
        tree.apply(&rt.flush());
        let root = rt.node_id(panel, 0).unwrap();
        Fixture {
            rt,
            tree,
            panel,
            root,
        }
    }

    fn flush(&mut self) -> Vec<Command> {
        let cmds = self.rt.flush();
        self.tree.apply(&cmds);
        cmds
    }

    /// The text of each child of the panel root, in order (`-` for a node
    /// without a direct text, a detail's own text is its first child's).
    fn shape(&self) -> Vec<String> {
        self.tree.children[&self.root]
            .iter()
            .map(|c| match self.tree.text.get(c) {
                Some(t) => t.clone(),
                None => self.tree.children[c]
                    .first()
                    .and_then(|g| self.tree.text.get(g))
                    .map(|t| format!("[{t}]"))
                    .unwrap_or_else(|| "-".into()),
            })
            .collect()
    }
}

#[test]
fn a_closed_panel_has_no_detail() {
    let f = Fixture::new();
    assert_eq!(f.shape(), ["Panel", "footer"]);
    assert_eq!(f.rt.live_instances(), 1);
}

#[test]
fn opening_mounts_the_detail_before_the_footer_with_its_props() {
    let mut f = Fixture::new();
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    let cmds = f.flush();
    assert_eq!(f.shape(), ["Panel", "[Hello world]", "footer"]);
    // Created with its final text: no update follows.
    assert!(
        cmds.iter()
            .all(|c| !matches!(c, Command::UpdateText { .. }))
    );
    assert_eq!(f.rt.live_instances(), 2);
}

#[test]
fn closing_removes_it_and_frees_the_instance() {
    let mut f = Fixture::new();
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    f.flush();
    f.rt.set(f.panel, OPEN, Value::Bool(false));
    let cmds = f.flush();
    assert_eq!(f.shape(), ["Panel", "footer"]);
    assert_eq!(cmds.len(), 1);
    assert!(matches!(cmds[0], Command::Remove { .. }));
    assert_eq!(f.rt.live_instances(), 1);
}

#[test]
fn a_prop_follows_the_parent_state_while_mounted() {
    let mut f = Fixture::new();
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    f.flush();
    f.rt.set(f.panel, NAME, Value::str("bevy"));
    let cmds = f.flush();
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    assert_eq!(f.shape(), ["Panel", "[Hello bevy]", "footer"]);
    // A prop write while closed costs nothing and shows up on open.
    f.rt.set(f.panel, OPEN, Value::Bool(false));
    f.flush();
    f.rt.set(f.panel, NAME, Value::str("again"));
    assert_eq!(f.flush(), vec![]);
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    f.flush();
    assert_eq!(f.shape(), ["Panel", "[Hello again]", "footer"]);
}

#[test]
fn open_and_close_inside_one_window_nets_nothing() {
    let mut f = Fixture::new();
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    f.rt.set(f.panel, OPEN, Value::Bool(false));
    assert_eq!(f.flush(), vec![]);
    // Close and reopen inside a window keeps the very same instance.
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    f.flush();
    let before: Vec<NodeId> = f.tree.children[&f.root].clone();
    f.rt.set(f.panel, OPEN, Value::Bool(false));
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    assert_eq!(f.flush(), vec![]);
    assert_eq!(f.tree.children[&f.root], before);
}

#[test]
fn the_child_keeps_its_own_state_across_parent_updates() {
    let mut f = Fixture::new();
    f.rt.set(f.panel, OPEN, Value::Bool(true));
    f.flush();
    let child_text = f.tree.children[&f.tree.children[&f.root][1]][0];
    f.rt.set(f.panel, NAME, Value::str("x"));
    let cmds = f.flush();
    assert!(
        matches!(&cmds[0], Command::UpdateText { id, .. } if *id == child_text),
        "the same node is updated in place"
    );
}

#[test]
fn an_always_mounted_child_with_no_before_appends() {
    let child = detail();
    let parent = Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("text", Some(0)).text("a"),
        ],
        children: vec![ChildBlock::new(0, child).prop(0, e::slot(0))],
        slots: vec![Value::str("hi")],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&parent, 0);
    let mut tree = Tree::default();
    tree.apply(&rt.flush());
    let root = rt.node_id(p, 0).unwrap();
    assert_eq!(tree.children[&root].len(), 2);
    let detail_root = tree.children[&root][1];
    assert_eq!(tree.text[&tree.children[&detail_root][0]], "hi");
}

#[test]
fn children_nest() {
    // Outer shows Middle while `a`; Middle shows Detail while its own `b`
    // (a prop from Outer's `b`).
    let middle = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        children: vec![
            ChildBlock::new(0, detail())
                .when(e::slot(0))
                .prop(0, e::lit("deep")),
        ],
        slots: vec![Value::Bool(false)],
        ..Default::default()
    });
    let outer = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        children: vec![
            ChildBlock::new(0, middle)
                .when(e::slot(0))
                .prop(0, e::slot(1)),
        ],
        slots: vec![Value::Bool(false), Value::Bool(false)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let o = rt.mount(&outer, 0);
    let mut tree = Tree::default();
    tree.apply(&rt.flush());
    assert_eq!(rt.live_instances(), 1);
    rt.set(o, 0, Value::Bool(true));
    rt.set(o, 1, Value::Bool(true));
    tree.apply(&rt.flush());
    assert_eq!(rt.live_instances(), 3, "outer, middle and detail");
    rt.set(o, 1, Value::Bool(false));
    tree.apply(&rt.flush());
    assert_eq!(rt.live_instances(), 2, "detail left with its prop");
    rt.unmount(o);
    tree.apply(&rt.flush());
    assert_eq!(rt.live_instances(), 0);
    assert!(tree.alive.is_empty());
}

#[test]
fn a_selected_row_shows_a_badge() {
    // Row: `{isSelected && <Badge />}`: the child's condition is the row's
    // selector flag, so selecting mounts one badge and nothing else changes.
    let badge = Compiled::new(Template {
        nodes: vec![TNode::new("text", None).text("✓")],
        ..Default::default()
    });
    let row = Compiled::new(Template {
        nodes: vec![TNode::new("button", None)],
        children: vec![ChildBlock::new(0, badge).when(e::slot(1))],
        // 0 = id, 1 = isSelected.
        slots: vec![Value::Num(0.0), Value::Bool(false)],
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
        (1..=100)
            .map(|i| ListItem::new(i, vec![Value::Num(i as f64)]))
            .collect(),
    )
    .unwrap();
    rt.flush();
    assert_eq!(rt.live_instances(), 101);
    rt.set(t, 0, Value::Num(7.0));
    let cmds = rt.flush();
    assert_eq!(rt.live_instances(), 102, "one badge");
    assert_eq!(cmds.len(), 2, "create + attach the badge: {cmds:?}");
    rt.set(t, 0, Value::Num(8.0));
    let cmds = rt.flush();
    assert_eq!(rt.live_instances(), 102, "the badge moved rows");
    assert!(matches!(cmds.first(), Some(Command::Remove { .. })));
    // The badge is removed with its row.
    rt.set_list(
        t,
        0,
        (1..=100).filter(|&i| i != 8).map(ListItem::same).collect(),
    )
    .unwrap();
    rt.flush();
    assert_eq!(rt.live_instances(), 100, "row 8 and its badge are gone");
}

#[test]
#[should_panic(expected = "is not a child of node 0")]
fn a_before_node_that_is_not_a_sibling_is_rejected() {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("node", Some(0)),
            TNode::new("text", Some(1)),
        ],
        children: vec![ChildBlock::new(0, detail()).before(2)],
        ..Default::default()
    });
}

#[test]
#[should_panic(expected = "reads slot 4")]
fn a_condition_reading_an_undeclared_slot_is_rejected() {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        children: vec![ChildBlock::new(0, detail()).when(e::slot(4))],
        ..Default::default()
    });
}

#[test]
#[should_panic(expected = "is a list's parent")]
fn a_child_under_a_list_parent_is_rejected() {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, detail())],
        children: vec![ChildBlock::new(0, detail())],
        ..Default::default()
    });
}

#[test]
fn a_ternary_is_two_complementary_children() {
    // `{on ? <A/> : <B/>}` before the footer: exactly one is ever mounted, and
    // flipping swaps them in one flush.
    let a = Compiled::new(Template {
        nodes: vec![TNode::new("text", None).text("A")],
        ..Default::default()
    });
    let b = Compiled::new(Template {
        nodes: vec![TNode::new("text", None).text("B")],
        ..Default::default()
    });
    let host = Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("text", Some(0)).text("footer"),
        ],
        children: vec![
            ChildBlock::new(0, a).before(1).when(e::slot(0)),
            ChildBlock::new(0, b).before(1).when(e::not(e::slot(0))),
        ],
        slots: vec![Value::Bool(true)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let h = rt.mount(&host, 0);
    let mut tree = Tree::default();
    tree.apply(&rt.flush());
    let root = rt.node_id(h, 0).unwrap();
    let shape = |tree: &Tree| -> Vec<String> {
        tree.children[&root]
            .iter()
            .map(|c| tree.text[c].clone())
            .collect()
    };
    assert_eq!(shape(&tree), ["A", "footer"]);
    rt.set(h, 0, Value::Bool(false));
    let cmds = rt.flush();
    tree.apply(&cmds);
    assert_eq!(shape(&tree), ["B", "footer"]);
    assert_eq!(rt.live_instances(), 2, "host and B");
    assert_eq!(cmds.len(), 3, "remove A, create B, place B: {cmds:?}");
    rt.set(h, 0, Value::Bool(true));
    tree.apply(&rt.flush());
    assert_eq!(shape(&tree), ["A", "footer"]);
}
