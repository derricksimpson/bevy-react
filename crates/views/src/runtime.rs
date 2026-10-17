//! The view runtime: mounts compiled templates, holds their state, and turns
//! state writes into the minimal command stream.
//!
//! Writes are batched: [`Runtime::set`] only marks the bindings that read the
//! slot, and [`Runtime::flush`] re-evaluates each marked binding once and
//! emits a command only when its value differs from the last one sent. So a
//! burst of writes costs one evaluation per affected hole, and a write that
//! ends where it started emits nothing.

use std::collections::HashMap;
use std::sync::Arc;

use crate::command::{Command, NodeId};
use crate::eval::eval;
use crate::ir::{JsRef, SlotId, Target, Template};
use crate::value::Value;

/// A template prepared for mounting: validated, with its slot → binding
/// index precomputed (the compiler knows every read statically).
#[derive(Debug)]
pub struct Compiled {
    template: Template,
    /// `slot_deps[slot]` = the bindings that read `slot`.
    slot_deps: Vec<Vec<u16>>,
    /// Per node, the handler props as they cross the wire (`"onClick"`).
    handler_props: Vec<Vec<String>>,
}

impl Compiled {
    /// Prepare `template`. Panics on IR the compiler should never emit (a
    /// parent after its child, a binding or read out of range).
    pub fn new(template: Template) -> Arc<Self> {
        for (i, n) in template.nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                assert!(usize::from(p) < i, "node {i}: parent {p} must come first");
            }
        }
        let mut slot_deps = vec![Vec::new(); template.slots.len()];
        for (b, binding) in template.bindings.iter().enumerate() {
            let node = match binding.target {
                Target::Text(n) | Target::Prop(n, _) | Target::Style(n, _) => n,
            };
            assert!(
                usize::from(node) < template.nodes.len(),
                "binding {b}: node {node}"
            );
            for slot in binding.expr.reads() {
                let deps = slot_deps.get_mut(usize::from(slot)).unwrap_or_else(|| {
                    panic!("binding {b} reads slot {slot}, which is not declared")
                });
                deps.push(b as u16);
            }
        }
        let handler_props = template
            .nodes
            .iter()
            .map(|n| n.handlers.iter().map(|(ev, _)| handler_prop(ev)).collect())
            .collect();
        Arc::new(Compiled {
            template,
            slot_deps,
            handler_props,
        })
    }

    pub fn template(&self) -> &Template {
        &self.template
    }
}

/// `"click"` → `"onClick"`.
fn handler_prop(event: &str) -> String {
    let mut chars = event.chars();
    match chars.next() {
        Some(first) => format!("on{}{}", first.to_uppercase(), chars.as_str()),
        None => "on".to_owned(),
    }
}

/// A mounted component instance.
pub type InstanceId = usize;

struct Instance {
    compiled: Arc<Compiled>,
    parent: NodeId,
    /// Bridge id of each template node.
    ids: Vec<NodeId>,
    slots: Vec<Value>,
    /// The value each binding last sent.
    sent: Vec<Value>,
    /// Bindings marked by a write since the last flush (and their flags).
    dirty: Vec<u16>,
    is_dirty: Vec<bool>,
}

/// The runtime for one island tree.
pub struct Runtime {
    next_id: NodeId,
    instances: Vec<Option<Instance>>,
    /// Instances with marked bindings, in first-write order.
    dirty: Vec<InstanceId>,
    /// Per node, its handlers (`(event, js)`; a node has one or two).
    handlers: HashMap<NodeId, Vec<(&'static str, JsRef)>>,
    out: Vec<Command>,
}

/// Where Rust-allocated ids start by default: the top half of the id space,
/// so islands inside a React tree never collide with React's `allocId`
/// (which counts up from 1).
pub const DEFAULT_ID_BASE: NodeId = 1 << 31;

impl Default for Runtime {
    fn default() -> Self {
        Self::with_id_base(DEFAULT_ID_BASE)
    }
}

impl Runtime {
    pub fn with_id_base(first_id: NodeId) -> Self {
        Runtime {
            next_id: first_id,
            instances: Vec::new(),
            dirty: Vec::new(),
            handlers: HashMap::new(),
            out: Vec::new(),
        }
    }

    /// Mount an instance of `compiled` under the bridge node `parent`. Its
    /// creates and appends are queued for the next [`flush`](Self::flush).
    pub fn mount(&mut self, compiled: &Arc<Compiled>, parent: NodeId) -> InstanceId {
        let t = &compiled.template;
        let ids: Vec<NodeId> = (0..t.nodes.len())
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                id
            })
            .collect();
        let slots = t.slots.clone();
        let sent: Vec<Value> = t.bindings.iter().map(|b| eval(&b.expr, &slots)).collect();

        // Creates carry the statics plus every binding's first value.
        for (ix, node) in t.nodes.iter().enumerate() {
            let mut props: Vec<(String, Value)> = node
                .props
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect();
            props.extend(
                compiled.handler_props[ix]
                    .iter()
                    .map(|p| (p.clone(), Value::Bool(true))),
            );
            let mut style = node.style.clone();
            let mut text = node.text.map(Arc::from);
            for (b, binding) in t.bindings.iter().enumerate() {
                let v = sent[b].clone();
                match binding.target {
                    Target::Text(n) if usize::from(n) == ix => {
                        text = Some(Arc::from(v.to_string()))
                    }
                    Target::Prop(n, key) if usize::from(n) == ix => upsert(&mut props, key, v),
                    Target::Style(n, key) if usize::from(n) == ix => {
                        upsert_static(&mut style, key, v)
                    }
                    _ => {}
                }
            }
            if !node.handlers.is_empty() {
                self.handlers.insert(ids[ix], node.handlers.clone());
            }
            self.out.push(Command::Create {
                id: ids[ix],
                kind: node.kind,
                props,
                style,
                text,
            });
        }
        // Children in template order, then the roots under the mount parent.
        for (ix, node) in t.nodes.iter().enumerate() {
            let parent_id = node.parent.map_or(parent, |p| ids[usize::from(p)]);
            if node.parent.is_some() {
                self.out.push(Command::Append {
                    parent: parent_id,
                    child: ids[ix],
                });
            }
        }
        for (ix, node) in t.nodes.iter().enumerate() {
            if node.parent.is_none() {
                self.out.push(Command::Append {
                    parent,
                    child: ids[ix],
                });
            }
        }

        let n_bindings = t.bindings.len();
        self.instances.push(Some(Instance {
            compiled: Arc::clone(compiled),
            parent,
            ids,
            slots,
            sent,
            dirty: Vec::new(),
            is_dirty: vec![false; n_bindings],
        }));
        self.instances.len() - 1
    }

    /// Write a state slot (a JS handler's `setX(value)`). Marks the bindings
    /// that read it; nothing is evaluated until [`flush`](Self::flush).
    pub fn set(&mut self, instance: InstanceId, slot: SlotId, value: Value) {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return;
        };
        let Some(cur) = inst.slots.get_mut(usize::from(slot)) else {
            return;
        };
        if *cur == value {
            return;
        }
        *cur = value;
        if inst.dirty.is_empty() {
            self.dirty.push(instance);
        }
        for &b in &inst.compiled.slot_deps[usize::from(slot)] {
            if !inst.is_dirty[usize::from(b)] {
                inst.is_dirty[usize::from(b)] = true;
                inst.dirty.push(b);
            }
        }
    }

    /// Read a slot (what a JS handler sees as the current state).
    pub fn get(&self, instance: InstanceId, slot: SlotId) -> Option<&Value> {
        self.instances
            .get(instance)?
            .as_ref()?
            .slots
            .get(usize::from(slot))
    }

    /// Re-evaluate every marked binding and return the commands queued since
    /// the last flush (mounts, unmounts and changed values), in order.
    pub fn flush(&mut self) -> Vec<Command> {
        for instance in std::mem::take(&mut self.dirty) {
            let Some(Some(inst)) = self.instances.get_mut(instance) else {
                continue;
            };
            let mut marked = std::mem::take(&mut inst.dirty);
            // Template order, not write order: a deterministic stream.
            marked.sort_unstable();
            for b in marked {
                inst.is_dirty[usize::from(b)] = false;
                let binding = &inst.compiled.template.bindings[usize::from(b)];
                let v = eval(&binding.expr, &inst.slots);
                if v == inst.sent[usize::from(b)] {
                    continue; // changed and changed back, or an equal result
                }
                let cmd = match binding.target {
                    Target::Text(n) => Command::UpdateText {
                        id: inst.ids[usize::from(n)],
                        text: Arc::from(v.to_string()),
                    },
                    Target::Prop(n, key) => Command::SetProp {
                        id: inst.ids[usize::from(n)],
                        key,
                        value: v.clone(),
                    },
                    Target::Style(n, key) => Command::SetStyle {
                        id: inst.ids[usize::from(n)],
                        key,
                        value: v.clone(),
                    },
                };
                inst.sent[usize::from(b)] = v;
                self.out.push(cmd);
            }
        }
        std::mem::take(&mut self.out)
    }

    /// Remove an instance: its roots leave the mount parent (Bevy despawns the
    /// subtree), its handlers are forgotten.
    pub fn unmount(&mut self, instance: InstanceId) {
        let Some(inst) = self.instances.get_mut(instance).and_then(Option::take) else {
            return;
        };
        for (ix, node) in inst.compiled.template.nodes.iter().enumerate() {
            if node.parent.is_none() {
                self.out.push(Command::Remove {
                    parent: inst.parent,
                    child: inst.ids[ix],
                });
            }
        }
        for id in &inst.ids {
            self.handlers.remove(id);
        }
        self.dirty.retain(|&i| i != instance);
    }

    /// The JS function to run for `event` on bridge node `node` (how a UI
    /// event from Bevy finds its compiled handler).
    pub fn handler(&self, node: NodeId, event: &str) -> Option<JsRef> {
        self.handlers
            .get(&node)?
            .iter()
            .find(|(ev, _)| *ev == event)
            .map(|(_, js)| *js)
    }

    /// The bridge id of template node `ix` in `instance`.
    pub fn node_id(&self, instance: InstanceId, ix: usize) -> Option<NodeId> {
        self.instances.get(instance)?.as_ref()?.ids.get(ix).copied()
    }
}

fn upsert(props: &mut Vec<(String, Value)>, key: &str, v: Value) {
    match props.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = v,
        None => props.push((key.to_owned(), v)),
    }
}

fn upsert_static(style: &mut Vec<(&'static str, Value)>, key: &'static str, v: Value) {
    match style.iter_mut().find(|(k, _)| *k == key) {
        Some(slot) => slot.1 = v,
        None => style.push((key, v)),
    }
}
