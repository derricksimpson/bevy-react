//! End to end: compiled views driving real Bevy entities.
//!
//! The runtime's commands go through [`OpAdapter`] into the core's real
//! `apply_js_ops` (the same system the JS bridge feeds), in the core's
//! headless op app. Run with `cargo test -p bevy_react_views --features bevy`.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_react_core::ReactNode;
use bevy_react_core::test_util::{children_of, ent, op_app};
use bevy_react_views::bevy::OpAdapter;
use bevy_react_views::{
    Action, Binding, Block, ChildBlock, Compiled, Dispatch, InstanceId, Key, ListItem, NodeId,
    Runtime, TNode, Target, Template, Value, e,
};

/// Flush the runtime, convert, send, and run one frame.
macro_rules! frame {
    ($app:expr, $tx:expr, $adapter:expr, $rt:expr) => {{
        let ops = $adapter.convert(&$rt.flush());
        $tx.send(ops).expect("the app is listening");
        $app.update();
    }};
}

fn text_of(app: &App, id: NodeId) -> String {
    app.world()
        .entity(ent(app, id))
        .get::<Text>()
        .expect("a <text>")
        .0
        .clone()
}

fn react_nodes(app: &mut App) -> usize {
    app.world_mut()
        .query::<&ReactNode>()
        .iter(app.world())
        .count()
}

/// `<button onClick={() => setCount(count + 1)} style={{ backgroundColor: count > 2 ? "#89b4fa" : "#45475a" }}>
///    <text>{`Count: ${count}`}</text></button>`
fn counter() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![
            TNode::new("button", None)
                .style("flexDirection", "column")
                .on_do(
                    "click",
                    vec![Action::set(0, e::add(e::slot(0), e::lit(1.0)))],
                ),
            TNode::new("text", Some(0)),
        ],
        bindings: vec![
            Binding {
                target: Target::Text(1),
                expr: e::concat([e::lit("Count: "), e::slot(0)]),
            },
            Binding {
                target: Target::Style(0, "backgroundColor"),
                expr: e::cond(
                    e::gt(e::slot(0), e::lit(2.0)),
                    e::lit("#89b4fa"),
                    e::lit("#45475a"),
                ),
            },
        ],
        slots: vec![Value::Num(0.0)],
        ..Default::default()
    })
}

fn srgb(color: Color) -> [f32; 3] {
    let c = color.to_srgba();
    [c.red, c.green, c.blue]
}

fn same_color(a: [f32; 3], hex: &str) -> bool {
    let b = Srgba::hex(hex).unwrap();
    (a[0] - b.red).abs() < 1e-3 && (a[1] - b.green).abs() < 1e-3 && (a[2] - b.blue).abs() < 1e-3
}

#[test]
fn a_counter_becomes_entities_and_clicks_update_them() {
    let (mut app, tx) = op_app();
    let mut adapter = OpAdapter::default();
    let mut rt = Runtime::with_id_base(1);
    let c = rt.mount(&counter(), 0);
    frame!(app, tx, adapter, rt);

    let (button, label) = (rt.node_id(c, 0).unwrap(), rt.node_id(c, 1).unwrap());
    assert_eq!(text_of(&app, label), "Count: 0");
    let node = app.world().entity(ent(&app, button)).get::<Node>().unwrap();
    assert_eq!(
        node.flex_direction,
        FlexDirection::Column,
        "the static style landed"
    );
    let bg = app
        .world()
        .entity(ent(&app, button))
        .get::<BackgroundColor>()
        .unwrap()
        .0;
    assert!(same_color(srgb(bg), "#45475a"));
    assert_eq!(children_of(&app, ent(&app, button)), vec![ent(&app, label)]);

    // Three clicks, all handled in Rust: the count and the color update.
    for _ in 0..3 {
        assert_eq!(rt.dispatch(button, "click"), Dispatch::Done);
    }
    frame!(app, tx, adapter, rt);
    assert_eq!(text_of(&app, label), "Count: 3");
    let bg = app
        .world()
        .entity(ent(&app, button))
        .get::<BackgroundColor>()
        .unwrap()
        .0;
    assert!(same_color(srgb(bg), "#89b4fa"), "the recolor landed");
}

/// A table of `<button><text>{`${id} ${label}`}</text></button>` rows.
fn table(windowed: bool) -> Arc<Compiled> {
    let row = Compiled::new(Template {
        nodes: vec![TNode::new("button", None), TNode::new("text", Some(0))],
        bindings: vec![Binding {
            target: Target::Text(1),
            expr: e::concat([e::slot(0), e::lit(" "), e::slot(1)]),
        }],
        slots: vec![Value::Num(0.0), Value::str("")],
        ..Default::default()
    });
    if windowed {
        Compiled::new(Template {
            nodes: vec![
                TNode::new("node", None),
                TNode::new("node", Some(0)),
                TNode::new("node", Some(0)),
            ],
            bindings: vec![
                Binding {
                    target: Target::Style(1, "height"),
                    expr: e::mul(e::slot(0), e::lit(20.0)),
                },
                Binding {
                    target: Target::Style(2, "height"),
                    expr: e::mul(e::slot(1), e::lit(20.0)),
                },
            ],
            blocks: vec![Block::list(0, row).windowed(2, 0, 1)],
            slots: vec![Value::Num(0.0), Value::Num(0.0)],
            ..Default::default()
        })
    } else {
        Compiled::new(Template {
            nodes: vec![TNode::new("node", None)],
            blocks: vec![Block::list(0, row)],
            ..Default::default()
        })
    }
}

fn item(id: i64) -> ListItem {
    ListItem::new(
        id,
        vec![Value::Num(id as f64), Value::str(&format!("row{id}"))],
    )
}

fn labels(app: &App, rt: &Runtime, t: InstanceId, list_node: NodeId) -> Vec<String> {
    let _ = (rt, t);
    children_of(app, ent(app, list_node))
        .into_iter()
        .filter_map(|row| children_of(app, row).first().copied())
        .filter_map(|t| app.world().entity(t).get::<Text>().map(|t| t.0.clone()))
        .collect()
}

#[test]
fn a_keyed_table_reorders_the_real_hierarchy() {
    let (mut app, tx) = op_app();
    let mut adapter = OpAdapter::default();
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(false), 0);
    rt.set_list(t, 0, (0..200).map(item).collect()).unwrap();
    frame!(app, tx, adapter, rt);
    let list = rt.node_id(t, 0).unwrap();
    let ids =
        |lo: i64, hi: i64| -> Vec<String> { (lo..hi).map(|i| format!("{i} row{i}")).collect() };
    assert_eq!(labels(&app, &rt, t, list), ids(0, 200));
    assert_eq!(react_nodes(&mut app), 1 + 400);

    // Swap two far-apart rows, drop every second, change a label.
    let mut order: Vec<i64> = (0..200).collect();
    order.swap(3, 190);
    rt.set_list(t, 0, order.iter().map(|i| ListItem::same(*i)).collect())
        .unwrap();
    frame!(app, tx, adapter, rt);
    let mut expected = ids(0, 200);
    expected.swap(3, 190);
    assert_eq!(labels(&app, &rt, t, list), expected);

    let kept: Vec<i64> = order.iter().copied().filter(|i| i % 2 == 0).collect();
    let mut items: Vec<ListItem> = kept.iter().map(|i| ListItem::same(*i)).collect();
    items[1] = ListItem::new(
        kept[1],
        vec![Value::Num(kept[1] as f64), Value::str("renamed")],
    );
    rt.set_list(t, 0, items).unwrap();
    frame!(app, tx, adapter, rt);
    let got = labels(&app, &rt, t, list);
    assert_eq!(got.len(), 100);
    assert_eq!(got[1], format!("{} renamed", kept[1]));
    assert_eq!(react_nodes(&mut app), 1 + 200, "removed rows despawned");
}

#[test]
fn a_windowed_table_spawns_only_the_rows_in_view() {
    let (mut app, tx) = op_app();
    let mut adapter = OpAdapter::default();
    let mut rt = Runtime::with_id_base(1);
    let t = rt.mount(&table(true), 0);
    rt.set_list(t, 0, (0..10_000).map(item).collect()).unwrap();
    rt.set_window(t, 0, 0, 30).unwrap();
    frame!(app, tx, adapter, rt);
    // The table, two spacers, and 30 rows of (button + text).
    assert_eq!(react_nodes(&mut app), 3 + 60);
    let (top, bottom) = (rt.node_id(t, 1).unwrap(), rt.node_id(t, 2).unwrap());
    let height = |app: &App, id| {
        app.world()
            .entity(ent(app, id))
            .get::<Node>()
            .unwrap()
            .height
    };
    assert_eq!(height(&app, top), Val::Px(0.0));
    assert_eq!(height(&app, bottom), Val::Px(9_970.0 * 20.0));

    // Scroll far down: still 30 rows, the spacers moved.
    rt.set_window(t, 0, 5000, 5030).unwrap();
    frame!(app, tx, adapter, rt);
    assert_eq!(react_nodes(&mut app), 3 + 60);
    assert_eq!(height(&app, top), Val::Px(5000.0 * 20.0));
    assert_eq!(height(&app, bottom), Val::Px(4_970.0 * 20.0));
    let list = rt.node_id(t, 0).unwrap();
    let shown = labels(&app, &rt, t, list);
    assert_eq!(shown.len(), 30);
    assert_eq!(shown[0], "5000 row5000");
    assert_eq!(shown[29], "5029 row5029");
}

#[test]
fn a_panel_mounts_and_unmounts_real_entities() {
    // `<node><button onClick={toggle}/>{open && <text>details</text>}<text>footer</text></node>`
    let detail = Compiled::new(Template {
        nodes: vec![TNode::new("text", None).text("details")],
        ..Default::default()
    });
    let panel = Compiled::new(Template {
        nodes: vec![
            TNode::new("node", None),
            TNode::new("button", Some(0)).on_do("click", vec![Action::set(0, e::not(e::slot(0)))]),
            TNode::new("text", Some(0)).text("footer"),
        ],
        children: vec![ChildBlock::new(0, detail).before(2).when(e::slot(0))],
        slots: vec![Value::Bool(false)],
        ..Default::default()
    });
    let (mut app, tx) = op_app();
    let mut adapter = OpAdapter::default();
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&panel, 0);
    frame!(app, tx, adapter, rt);
    let (root, button) = (rt.node_id(p, 0).unwrap(), rt.node_id(p, 1).unwrap());
    let kids = |app: &App| -> Vec<String> {
        children_of(app, ent(app, root))
            .into_iter()
            .map(|c| {
                app.world()
                    .entity(c)
                    .get::<Text>()
                    .map_or("button".to_owned(), |t| t.0.clone())
            })
            .collect()
    };
    assert_eq!(kids(&app), ["button", "footer"]);
    rt.dispatch(button, "click");
    frame!(app, tx, adapter, rt);
    assert_eq!(
        kids(&app),
        ["button", "details", "footer"],
        "inserted before the footer"
    );
    rt.dispatch(button, "click");
    frame!(app, tx, adapter, rt);
    assert_eq!(kids(&app), ["button", "footer"]);
    assert_eq!(react_nodes(&mut app), 3);
    let _ = Key::Int(0);
}

/// Timing, not a correctness test: `cargo test -p bevy_react_views --features
/// bevy --test bevy_e2e -- --ignored --nocapture`. The harness runs only
/// `apply_js_ops` (no layout or render), so this is the apply leg alone.
#[test]
#[ignore = "timing"]
fn apply_cost_of_a_10k_table_with_and_without_a_window() {
    use std::time::Instant;
    for windowed in [false, true] {
        let (mut app, tx) = op_app();
        let mut adapter = OpAdapter::default();
        let mut rt = Runtime::with_id_base(1);
        let t = rt.mount(&table(windowed), 0);
        rt.set_list(t, 0, (0..10_000).map(item).collect()).unwrap();
        if windowed {
            rt.set_window(t, 0, 0, 30).unwrap();
        }
        let t0 = Instant::now();
        let cmds = rt.flush();
        let t1 = Instant::now();
        let ops = adapter.convert(&cmds);
        let t2 = Instant::now();
        tx.send(ops).unwrap();
        app.update();
        let t3 = Instant::now();
        let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1000.0;
        println!(
            "{:<10} runtime {:>7.2} ms | adapter {:>7.2} ms | apply_js_ops {:>7.2} ms | {} commands, {} entities",
            if windowed { "windowed" } else { "all rows" },
            ms(t0, t1),
            ms(t1, t2),
            ms(t2, t3),
            cmds.len(),
            react_nodes(&mut app),
        );
    }
}
