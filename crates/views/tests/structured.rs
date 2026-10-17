//! Structured style values (objects and arrays) and the runtime's thread
//! safety.

use bevy_react_views::{Binding, Command, Compiled, Runtime, TNode, Target, Template, Value, e};

#[test]
fn a_style_object_is_built_from_state_and_only_resent_when_it_differs() {
    // `<node style={{ transform: { rotate: `${deg}deg` }, boxShadow: [{ blur: 4 }] }} />`
    let node = Compiled::new(Template {
        nodes: vec![TNode::new("node", None).style(
            "boxShadow",
            Value::list([Value::map([("blur", Value::Num(4.0))])]),
        )],
        bindings: vec![Binding {
            target: Target::Style(0, "transform"),
            expr: e::map([("rotate", e::concat([e::slot(0), e::lit("deg")]))]),
        }],
        slots: vec![Value::Num(10.0), Value::Num(0.0)],
        ..Default::default()
    });
    let mut rt = Runtime::with_id_base(1);
    let n = rt.mount(&node, 0);
    let created = rt.flush();
    let Command::Create { style, .. } = &created[0] else {
        panic!("a create first: {created:?}");
    };
    assert!(style.contains(&(
        "boxShadow",
        Value::list([Value::map([("blur", Value::Num(4.0))])])
    )));
    assert!(style.contains(&("transform", Value::map([("rotate", Value::str("10deg"))]))));

    rt.set(n, 0, Value::Num(45.0));
    assert_eq!(
        rt.flush(),
        vec![Command::SetStyle {
            id: rt.node_id(n, 0).unwrap(),
            key: "transform",
            value: Value::map([("rotate", Value::str("45deg"))]),
        }]
    );
    // A write that rebuilds an equal object sends nothing.
    rt.set(n, 1, Value::Num(1.0)); // unrelated
    assert_eq!(rt.flush(), vec![]);
    rt.set(n, 0, Value::Num(45.0));
    assert_eq!(rt.flush(), vec![]);
}

#[test]
fn the_runtime_can_live_in_a_bevy_resource() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Runtime>();
    assert_send_sync::<Compiled>();
    assert_send_sync::<Value>();
}
