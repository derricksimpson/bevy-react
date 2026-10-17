//! The expression language in a real component:
//!
//! ```tsx
//! function Progress({ value, max }) {
//!   const pct = Math.min(Math.max(value / max, 0), 1) * 100;
//!   return (
//!     <node style={{ width: `${pct}%` }}>
//!       <text>{`${pct.toFixed(1)}%${pct >= 100 ? " done" : ""}`}</text>
//!     </node>
//!   );
//! }
//! ```

use std::sync::Arc;

use bevy_react_views::{Binding, Command, Compiled, Runtime, TNode, Target, Template, Value, e};

fn pct() -> bevy_react_views::Expr {
    // clamp(value / max, 0, 1) * 100
    e::mul(
        e::clamp(e::div(e::slot(0), e::slot(1)), e::lit(0.0), e::lit(1.0)),
        e::lit(100.0),
    )
}

fn progress() -> Arc<Compiled> {
    Compiled::new(Template {
        nodes: vec![TNode::new("node", None), TNode::new("text", Some(0))],
        bindings: vec![
            Binding {
                target: Target::Style(0, "width"),
                expr: e::concat([pct(), e::lit("%")]),
            },
            Binding {
                target: Target::Text(1),
                expr: e::concat([
                    e::fixed(pct(), 1),
                    e::lit("%"),
                    e::cond(e::ge(pct(), e::lit(100.0)), e::lit(" done"), e::lit("")),
                ]),
            },
        ],
        slots: vec![Value::Num(0.0), Value::Num(200.0)],
        ..Default::default()
    })
}

fn text_of(cmds: &[Command]) -> Option<String> {
    cmds.iter().find_map(|c| match c {
        Command::UpdateText { text, .. } => Some(text.to_string()),
        _ => None,
    })
}

fn width_of(cmds: &[Command]) -> Option<Value> {
    cmds.iter().find_map(|c| match c {
        Command::SetStyle {
            key: "width",
            value,
            ..
        } => Some(value.clone()),
        _ => None,
    })
}

#[test]
fn a_progress_bar_tracks_its_value() {
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&progress(), 0);
    let created = rt.flush();
    assert!(created.iter().any(|c| matches!(c,
        Command::Create { text: Some(t), .. } if &**t == "0.0%")));

    rt.set(p, 0, Value::Num(50.0));
    let cmds = rt.flush();
    assert_eq!(text_of(&cmds).as_deref(), Some("25.0%"));
    assert_eq!(width_of(&cmds), Some(Value::str("25%")));

    rt.set(p, 0, Value::Num(1.0));
    assert_eq!(text_of(&rt.flush()).as_deref(), Some("0.5%"));
}

#[test]
fn the_bar_clamps_and_reports_done() {
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&progress(), 0);
    rt.flush();
    rt.set(p, 0, Value::Num(500.0));
    let cmds = rt.flush();
    assert_eq!(text_of(&cmds).as_deref(), Some("100.0% done"));
    assert_eq!(width_of(&cmds), Some(Value::str("100%")));
    // Past full, more value changes nothing visible.
    rt.set(p, 0, Value::Num(900.0));
    assert_eq!(rt.flush(), vec![]);
    rt.set(p, 0, Value::Num(-40.0));
    let cmds = rt.flush();
    assert_eq!(text_of(&cmds).as_deref(), Some("0.0%"));
    assert_eq!(width_of(&cmds), Some(Value::str("0%")));
}

#[test]
fn a_zero_max_is_not_a_crash() {
    let mut rt = Runtime::with_id_base(1);
    let p = rt.mount(&progress(), 0);
    rt.flush();
    rt.set(p, 1, Value::Num(0.0)); // 0 / 0 = NaN; clamp keeps NaN
    let cmds = rt.flush();
    assert_eq!(text_of(&cmds).as_deref(), Some("NaN%"));
}
