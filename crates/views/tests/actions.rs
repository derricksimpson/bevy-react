//! Rust-side handlers: clicks that never leave Rust.
//!
//! ```tsx
//! <button onClick={() => setCount(count + 1)}>        // counter
//! <button onClick={() => setOpen(!open)}>             // toggle a panel
//! <Row onClick={() => select(row.id)} />              // select a row
//! <button onClick={() => { setN(n + 1); track(); }}>  // Rust, then JS
//! ```

use std::sync::Arc;

use bevy_react_views::{
    Action, Binding, Block, ChildBlock, Command, Compiled, Dispatch, Key, ListItem, Runtime, TNode,
    Target, Template, Value, e,
};

fn text(id: u32, s: &str) -> Command {
    Command::UpdateText {
        id,
        text: Arc::from(s),
    }
}

/// `<button onClick={() => setCount(count + 1)}><text>{`Count: ${count}`}</text></button>`
fn counter() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None).on_do(
                "click",
                vec![Action::set(0, e::add(e::slot(0), e::lit(1.0)))],
            ),
            TNode::new("text", Some(0)),
        ],
        bindings: vec![Binding {
            target: Target::Text(1),
            expr: e::concat([e::lit("Count: "), e::slot(0)]),
        }],
        slots: vec![Value::Num(0.0)],
        ..Default::default()
    })
}

#[test]
fn a_counter_runs_without_js() {
    let mut rt = Runtime::with_id_base(1);
    let c = rt.mount(&counter(), 0);
    rt.flush();
    let button = rt.node_id(c, 0).unwrap();
    let label = rt.node_id(c, 1).unwrap();
    assert_eq!(rt.dispatch(button, "click"), Dispatch::Done);
    assert_eq!(rt.flush(), vec![text(label, "Count: 1")]);
    for _ in 0..4 {
        rt.dispatch(button, "click");
    }
    assert_eq!(
        rt.flush(),
        vec![text(label, "Count: 5")],
        "five clicks, one op"
    );
    assert_eq!(rt.get(c, 0), Some(&Value::Num(5.0)));
}

#[test]
fn unknown_nodes_and_events_do_nothing() {
    let mut rt = Runtime::with_id_base(1);
    let c = rt.mount(&counter(), 0);
    rt.flush();
    let button = rt.node_id(c, 0).unwrap();
    let label = rt.node_id(c, 1).unwrap();
    assert_eq!(rt.dispatch(button, "pointerDown"), Dispatch::None);
    assert_eq!(
        rt.dispatch(label, "click"),
        Dispatch::None,
        "no handler on the label"
    );
    assert_eq!(rt.dispatch(9999, "click"), Dispatch::None);
    assert_eq!(rt.flush(), vec![]);
}

#[test]
fn a_click_toggles_a_panel() {
    // `<node><button onClick={() => setOpen(!open)}/>{open && <Detail/>}</node>`
    let detail = Compiled::new(Template {
        nodes: vec![TNode::new("text", None).text("details")],
        ..Default::default()
    });
    let panel = Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("button", Some(0)).on_do("click", vec![Action::set(0, e::not(e::slot(0)))]),
        ],
        children: vec![ChildBlock::new(0, detail).when(e::slot(0))],
        slots: vec![Value::Bool(false)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&panel, 0);
    rt.flush();
    let button = rt.node_id(p, 1).unwrap();
    rt.dispatch(button, "click");
    let cmds = rt.flush();
    assert_eq!(rt.live_instances(), 2, "opened");
    assert!(
        cmds.iter()
            .any(|c| matches!(c, Command::Create { kind: "text", .. }))
    );
    rt.dispatch(button, "click");
    let cmds = rt.flush();
    assert_eq!(rt.live_instances(), 1, "closed");
    assert!(matches!(cmds[..], [Command::Remove { .. }]));
}

/// A table of rows whose click selects them: `onClick={() => setSelected(row.id)}`.
fn selectable_table() -> Arc<Compiled> {
    let row = Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None)
                // up: 1 = the table; the id comes from the row's own slot 0.
                .on_do("click", vec![Action::set_owner(0, e::slot(0))]),
            TNode::new("text", Some(0)),
        ],
        bindings: vec![
            Binding {
                target: Target::Text(1),
                expr: e::slot(0),
            },
            Binding {
                target: Target::Style(0, "backgroundColor"),
                expr: e::cond(e::slot(1), e::lit("sel"), e::lit("row")),
            },
        ],
        slots: vec![Value::Num(0.0), Value::Bool(false)],
        ..Default::default()
    });
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row).select(0, 0, 1)],
        slots: vec![Value::Null],
        ..Default::default()
    })
}

#[test]
fn clicking_a_row_selects_it_without_js() {
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&selectable_table(), 0);
    rt.set_list(
        t,
        0,
        (1..=1000)
            .map(|i| ListItem::new(i, vec![Value::Num(i as f64)]))
            .collect(),
    )
    .unwrap();
    rt.flush();
    let button_of = |rt: &Runtime, id: i64| {
        let row = rt.list_item(t, 0, &Key::Int(id)).unwrap();
        rt.node_id(row, 0).unwrap()
    };
    let recolor = |rt: &Runtime, id: i64, color: &str| {
        let row = rt.list_item(t, 0, &Key::Int(id)).unwrap();
        Command::SetStyle {
            id: rt.node_id(row, 0).unwrap(),
            key: "backgroundColor",
            value: Value::str(color),
        }
    };
    let b = button_of(&rt, 700);
    assert_eq!(rt.dispatch(b, "click"), Dispatch::Done);
    assert_eq!(rt.flush(), vec![recolor(&rt, 700, "sel")]);
    // Click another row: the old one clears, the new one lights.
    let b = button_of(&rt, 3);
    rt.dispatch(b, "click");
    assert_eq!(
        rt.flush(),
        vec![recolor(&rt, 700, "row"), recolor(&rt, 3, "sel")]
    );
    assert_eq!(
        rt.get(t, 0),
        Some(&Value::Num(3.0)),
        "the table holds the selection"
    );
}

#[test]
fn a_handler_reads_one_state_for_all_its_actions() {
    // `onClick={() => { setA(b); setB(a); }}` swaps, as in React.
    let swap = Compiled::new(Template {
        nodes: vec![TNode::new("button", None).on_do(
            "click",
            vec![Action::set(0, e::slot(1)), Action::set(1, e::slot(0))],
        )],
        slots: vec![Value::Num(1.0), Value::Num(2.0)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let s = rt.mount(&swap, 0);
    rt.flush();
    rt.dispatch(rt.node_id(s, 0).unwrap(), "click");
    assert_eq!(rt.get(s, 0), Some(&Value::Num(2.0)));
    assert_eq!(rt.get(s, 1), Some(&Value::Num(1.0)));
}

#[test]
fn a_handler_can_finish_in_js() {
    // `onClick={() => { setN(n + 1); track(n); }}`: Rust first, then JS.
    let node = Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None)
                .on_do_then(
                    "click",
                    vec![Action::set(0, e::add(e::slot(0), e::lit(1.0)))],
                    42,
                )
                .on("pointerDown", 7),
        ],
        slots: vec![Value::Num(0.0)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let n = rt.mount(&node, 0);
    rt.flush();
    let button = rt.node_id(n, 0).unwrap();
    assert_eq!(
        rt.dispatch(button, "click"),
        Dispatch::Js {
            js: 42,
            instance: n
        }
    );
    assert_eq!(
        rt.get(n, 0),
        Some(&Value::Num(1.0)),
        "JS runs after the actions took effect"
    );
    assert_eq!(
        rt.dispatch(button, "pointerDown"),
        Dispatch::Js { js: 7, instance: n }
    );
    assert_eq!(rt.handler(button, "click"), Some((42, n)));
}

#[test]
fn writing_above_the_root_is_ignored() {
    let mut rt = Runtime::with_id_base(1);
    let c = Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None).on_do("click", vec![Action::set_owner(0, e::lit(1.0))]),
        ],
        ..Default::default()
    });
    let m = rt.mount(&c, 0);
    rt.flush();
    assert_eq!(
        rt.dispatch(rt.node_id(m, 0).unwrap(), "click"),
        Dispatch::Done
    );
    assert_eq!(rt.flush(), vec![]);
}

#[test]
fn events_find_their_rows_after_heavy_churn() {
    // Enough rows going away that the id-range table sweeps itself.
    let row = Compiled::new(Template {
        nodes: vec![TNode::new("button", None).on("click", 9)],
        slots: vec![Value::Num(0.0)],
        ..Default::default()
    });
    let table = Compiled::new(Template {
        nodes: vec![TNode::new("node", None)],
        blocks: vec![Block::list(0, row)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table, 0);
    let all = |ids: std::ops::Range<i64>| {
        ids.map(|i| ListItem::new(i, vec![Value::Num(i as f64)]))
            .collect::<Vec<_>>()
    };
    rt.set_list(t, 0, all(0..4000)).unwrap();
    rt.flush();
    let dropped: Vec<u32> = (0..2500)
        .map(|i| {
            let r = rt.list_item(t, 0, &Key::Int(i)).unwrap();
            rt.node_id(r, 0).unwrap()
        })
        .collect();
    rt.set_list(t, 0, (2500..4000).map(ListItem::same).collect())
        .unwrap();
    rt.flush();
    for node in dropped {
        assert_eq!(rt.dispatch(node, "click"), Dispatch::None);
    }
    for i in [2500, 3000, 3999] {
        let r = rt.list_item(t, 0, &Key::Int(i)).unwrap();
        let b = rt.node_id(r, 0).unwrap();
        assert_eq!(rt.dispatch(b, "click"), Dispatch::Js { js: 9, instance: r });
    }
    // New rows after the sweep still register.
    rt.set_list(t, 0, {
        let mut v: Vec<ListItem> = (2500..4000).map(ListItem::same).collect();
        v.extend(all(5000..5010));
        v
    })
    .unwrap();
    rt.flush();
    let r = rt.list_item(t, 0, &Key::Int(5005)).unwrap();
    assert!(matches!(
        rt.dispatch(rt.node_id(r, 0).unwrap(), "click"),
        Dispatch::Js { js: 9, .. }
    ));
}

#[test]
#[should_panic(expected = "reads slot 5")]
fn an_action_reading_an_undeclared_slot_is_rejected() {
    Compiled::new(Template {
        nodes: vec![TNode::new("button", None).on_do("click", vec![Action::set(0, e::slot(5))])],
        slots: vec![Value::Null],
        ..Default::default()
    });
}
