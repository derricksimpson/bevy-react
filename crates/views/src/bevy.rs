//! The adapter from [`Command`]s to `bevy_react_core`'s wire ops.
//!
//! The runtime's commands mirror the wire ops one-to-one, so this is the whole
//! integration: a command stream becomes the `Vec<Op>` the core's
//! `apply_js_ops` already consumes, and everything downstream (the style
//! writers, transitions, layers, picking) is untouched.
//!
//! This first adapter builds each op's props the way the JS bridge does, as
//! JSON decoded against the node's element. That keeps it exactly as
//! faithful as the JS path while the shape of the thing is proven; decoding a
//! template's static props once and cloning them per instance is the obvious
//! next step, and needs no change on the runtime's side.

use std::collections::HashMap;

use bevy_react_core::protocol::op::Op;
use bevy_react_core::protocol::props::Props;
use serde_json::{Map, Value as Json};

use crate::command::{Command, NodeId};
use crate::value::Value;

/// Converts command streams to ops, remembering each node's element kind (an
/// update's props decode against it).
#[derive(Default)]
pub struct OpAdapter {
    kinds: HashMap<NodeId, &'static str>,
}

impl OpAdapter {
    /// The ops for `commands`, in order. Must run on the thread that holds the
    /// app's element registry (the one the JS bridge decodes on).
    pub fn convert(&mut self, commands: &[Command]) -> Vec<Op> {
        commands.iter().map(|c| self.op(c)).collect()
    }

    fn op(&mut self, command: &Command) -> Op {
        match command {
            Command::Create {
                id,
                kind,
                props,
                style,
                text,
            } => {
                self.kinds.insert(*id, kind);
                let mut json: Map<String, Json> = props
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), json(v)))
                    .collect();
                if !style.is_empty() {
                    let style: Map<String, Json> = style
                        .iter()
                        .map(|(k, v)| ((*k).to_owned(), json(v)))
                        .collect();
                    json.insert("style".into(), Json::Object(style));
                }
                Op::Create {
                    id: *id,
                    kind: (*kind).into(),
                    props: Props::decode_for(kind, Json::Object(json)),
                    text: text.as_ref().map(|t| t.to_string()),
                }
            }
            Command::Append { parent, child } => Op::Append {
                parent: *parent,
                child: *child,
            },
            Command::Insert {
                parent,
                child,
                before,
            } => Op::Insert {
                parent: *parent,
                child: *child,
                before: *before,
            },
            Command::Remove { parent, child } => Op::Remove {
                parent: *parent,
                child: *child,
            },
            Command::SetProp { id, key, value } => self.update(
                *id,
                [((*key).to_owned(), json(value))].into_iter().collect(),
            ),
            Command::SetStyle { id, key, value } => {
                let style: Map<String, Json> =
                    [((*key).to_owned(), json(value))].into_iter().collect();
                self.update(
                    *id,
                    [("style".to_owned(), Json::Object(style))]
                        .into_iter()
                        .collect(),
                )
            }
            Command::UpdateText { id, text } => Op::UpdateText {
                id: *id,
                text: text.to_string(),
            },
        }
    }

    /// A delta update carrying only `props`.
    fn update(&self, id: NodeId, props: Map<String, Json>) -> Op {
        let kind = self.kinds.get(&id).copied().unwrap_or("node");
        Op::Update {
            id,
            props: Props::decode_for(kind, Json::Object(props)),
            unset: Vec::new(),
            style_unset: Vec::new(),
        }
    }
}

/// A binding value as the wire carries it: integral numbers as integers
/// (lengths and counts), other numbers as floats, `NaN`/`Infinity` as `null`
/// (what `JSON.stringify` does).
fn json(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Json::from(*n as i64),
        Value::Num(n) => serde_json::Number::from_f64(*n).map_or(Json::Null, Json::Number),
        Value::Str(s) => Json::String(s.to_string()),
        Value::List(items) => Json::Array(items.iter().map(json).collect()),
        Value::Map(entries) => Json::Object(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), json(v)))
                .collect(),
        ),
    }
}
