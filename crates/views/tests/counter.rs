//! The first compiled view: a counter, hand-written as the IR the compiler
//! would emit for
//!
//! ```tsx
//! function Counter() {
//!   const [count, setCount] = useState(0);
//!   return (
//!     <node style={{ flexDirection: "column" }}>
//!       <text>{`Count: ${count}`}</text>
//!       <button style={{ backgroundColor: count > 3 ? "red" : "blue" }}
//!               onClick={() => setCount(count + 1)}>
//!         <text>+1</text>
//!       </button>
//!     </node>
//!   );
//! }
//! ```
//!
//! Every test pins the exact command stream: the same ops React would send,
//! and nothing else.

use std::sync::Arc;

use bevy_react_views::{Binding, Command, Compiled, Runtime, TNode, Target, Template, Value, e};

const COUNT: u16 = 0;
const ROOT: u32 = 0;
// Template node indices.
const LABEL: usize = 1;
const BUTTON: usize = 2;
// The click handler's JS function id.
const INC: u32 = 7;

fn counter() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None).style("flexDirection", "column"),
            TNode::new("text", Some(0)),
            TNode::new("button", Some(0)).on("click", INC),
            TNode::new("text", Some(2)).text("+1"),
        ],
        bindings: vec![
            Binding {
                target: Target::Text(1),
                expr: e::concat([e::lit("Count: "), e::slot(COUNT)]),
            },
            Binding {
                target: Target::Style(2, "backgroundColor"),
                expr: e::cond(
                    e::gt(e::slot(COUNT), e::lit(3.0)),
                    e::lit("red"),
                    e::lit("blue"),
                ),
            },
        ],
        slots: vec![Value::Num(0.0)],
        ..Default::default()
    })
}

/// A runtime with ids from 100, so expected streams read easily.
fn runtime() -> Runtime {
    Runtime::with_id_base(100)
}

fn text(id: u32, s: &str) -> Command {
    Command::UpdateText {
        id,
        text: Arc::from(s),
    }
}

#[test]
fn mount_emits_the_tree_once() {
    let mut rt = runtime();
    rt.mount(&counter(), ROOT);
    assert_eq!(
        rt.flush(),
        vec![
            Command::Create {
                id: 100,
                kind: "node",
                props: vec![],
                style: vec![("flexDirection", Value::str("column"))],
                text: None,
            },
            Command::Create {
                id: 101,
                kind: "text",
                props: vec![],
                style: vec![],
                text: Some(Arc::from("Count: 0")),
            },
            Command::Create {
                id: 102,
                kind: "button",
                props: vec![("onClick".to_owned(), Value::Bool(true))],
                style: vec![("backgroundColor", Value::str("blue"))],
                text: None,
            },
            Command::Create {
                id: 103,
                kind: "text",
                props: vec![],
                style: vec![],
                text: Some(Arc::from("+1")),
            },
            Command::Append {
                parent: 100,
                child: 101
            },
            Command::Append {
                parent: 100,
                child: 102
            },
            Command::Append {
                parent: 102,
                child: 103
            },
            Command::Append {
                parent: ROOT,
                child: 100
            },
        ]
    );
    assert_eq!(rt.flush(), vec![], "a second flush has nothing to send");
}

#[test]
fn a_write_updates_only_the_holes_whose_value_changed() {
    let mut rt = runtime();
    let c = rt.mount(&counter(), ROOT);
    rt.flush();

    // 0 → 1: the label changes; the color stays "blue", so no style op.
    rt.set(c, COUNT, Value::Num(1.0));
    assert_eq!(rt.flush(), vec![text(101, "Count: 1")]);

    // 3 → 4 crosses the threshold: label and color.
    rt.set(c, COUNT, Value::Num(4.0));
    assert_eq!(
        rt.flush(),
        vec![
            text(101, "Count: 4"),
            Command::SetStyle {
                id: 102,
                key: "backgroundColor",
                value: Value::str("red")
            },
        ]
    );
}

#[test]
fn writes_batch_until_flush() {
    let mut rt = runtime();
    let c = rt.mount(&counter(), ROOT);
    rt.flush();
    for n in 1..=3 {
        rt.set(c, COUNT, Value::Num(f64::from(n)));
    }
    assert_eq!(
        rt.flush(),
        vec![text(101, "Count: 3")],
        "three writes, one op"
    );
}

#[test]
fn a_write_that_ends_where_it_started_sends_nothing() {
    let mut rt = runtime();
    let c = rt.mount(&counter(), ROOT);
    rt.flush();
    rt.set(c, COUNT, Value::Num(0.0));
    assert_eq!(rt.flush(), vec![], "same value");
    rt.set(c, COUNT, Value::Num(5.0));
    rt.set(c, COUNT, Value::Num(0.0));
    assert_eq!(
        rt.flush(),
        vec![],
        "changed and changed back before the flush"
    );
}

#[test]
fn ui_events_find_their_js_handler() {
    let mut rt = runtime();
    let c = rt.mount(&counter(), ROOT);
    rt.flush();
    let button = rt.node_id(c, BUTTON).unwrap();
    assert_eq!(rt.handler(button, "click"), Some((INC, c)));
    assert_eq!(rt.handler(button, "pointerDown"), None);
    assert_eq!(rt.handler(rt.node_id(c, LABEL).unwrap(), "click"), None);

    // What the JS handler does with it: read state, write state.
    let next = rt.get(c, COUNT).unwrap().num() + 1.0;
    rt.set(c, COUNT, Value::Num(next));
    assert_eq!(rt.flush(), vec![text(101, "Count: 1")]);
}

#[test]
fn instances_keep_their_own_state() {
    let mut rt = runtime();
    let compiled = counter();
    let a = rt.mount(&compiled, ROOT);
    let b = rt.mount(&compiled, ROOT);
    rt.flush();
    rt.set(b, COUNT, Value::Num(2.0));
    assert_eq!(
        rt.flush(),
        vec![text(rt.node_id(b, LABEL).unwrap(), "Count: 2")]
    );
    assert_eq!(rt.get(a, COUNT), Some(&Value::Num(0.0)));
}

#[test]
fn unmount_removes_the_roots_and_forgets_handlers() {
    let mut rt = runtime();
    let c = rt.mount(&counter(), ROOT);
    rt.flush();
    let button = rt.node_id(c, BUTTON).unwrap();
    rt.set(c, COUNT, Value::Num(9.0)); // pending, never sent
    rt.unmount(c);
    assert_eq!(
        rt.flush(),
        vec![Command::Remove {
            parent: ROOT,
            child: 100
        }]
    );
    assert_eq!(rt.handler(button, "click"), None);
    rt.set(c, COUNT, Value::Num(1.0));
    assert_eq!(rt.flush(), vec![], "writes to a gone instance are ignored");
}

#[test]
#[should_panic(expected = "reads slot 3")]
fn a_binding_reading_an_undeclared_slot_is_rejected() {
    Compiled::new(Template {
        nodes: vec![TNode::new("text", None)],
        bindings: vec![Binding {
            target: Target::Text(0),
            expr: e::slot(3),
        }],
        slots: vec![],
        ..Default::default()
    });
}
