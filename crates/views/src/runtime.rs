//! The view runtime: mounts compiled templates, holds their state, and turns
//! state writes into the minimal command stream.
//!
//! Writes are batched: [`Runtime::set`] only marks the bindings that read the
//! slot, and [`Runtime::flush`] re-evaluates each marked binding once and
//! emits a command only when its value differs from the last one sent. So a
//! burst of writes costs one evaluation per affected hole, and a write that
//! ends where it started emits nothing. Keyed lists are in [`crate::list`].

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::batch::{Clock, instant_clock};
use crate::command::{Command, NodeId};
use crate::eval::eval;
use crate::ir::{Block, JsRef, SlotId, Target, Template};
use crate::list::ListState;
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
    /// parent after its child, a binding or read out of range, a list that
    /// shares its parent with static children or has a multi-root item).
    pub fn new(template: Template) -> Arc<Self> {
        let nodes = &template.nodes;
        for (i, n) in nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                assert!(usize::from(p) < i, "node {i}: parent {p} must come first");
            }
        }
        let mut slot_deps = vec![Vec::new(); template.slots.len()];
        for (b, binding) in template.bindings.iter().enumerate() {
            let node = match binding.target {
                Target::Text(n) | Target::Prop(n, _) | Target::Style(n, _) => n,
            };
            assert!(usize::from(node) < nodes.len(), "binding {b}: node {node}");
            for slot in binding.expr.reads() {
                let deps = slot_deps.get_mut(usize::from(slot)).unwrap_or_else(|| {
                    panic!("binding {b} reads slot {slot}, which is not declared")
                });
                deps.push(b as u16);
            }
        }
        for Block::For { parent, item } in &template.blocks {
            assert!(usize::from(*parent) < nodes.len(), "list parent {parent}");
            assert!(
                !nodes.iter().any(|n| n.parent == Some(*parent)),
                "list parent {parent} has static children"
            );
            let roots = item
                .template
                .nodes
                .iter()
                .filter(|n| n.parent.is_none())
                .count();
            assert!(
                roots == 1 && item.template.nodes[0].parent.is_none(),
                "list item needs one root, node 0"
            );
        }
        let handler_props = nodes
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

pub(crate) struct Instance {
    pub(crate) compiled: Arc<Compiled>,
    pub(crate) parent: NodeId,
    /// Bridge id of each template node.
    pub(crate) ids: Vec<NodeId>,
    slots: Vec<Value>,
    /// The value each binding last sent.
    sent: Vec<Value>,
    /// Bindings marked by a write since the last flush (and their flags).
    dirty: Vec<u16>,
    is_dirty: Vec<bool>,
    /// One per `Block::For`, in template order.
    pub(crate) lists: Vec<ListState>,
}

/// A node's handlers: its instance, and `(event, js)` pairs (one or two).
type NodeHandlers = (InstanceId, Vec<(&'static str, JsRef)>);

/// The runtime for one island tree.
pub struct Runtime {
    next_id: NodeId,
    pub(crate) instances: Vec<Option<Instance>>,
    /// Instances with marked bindings, in first-write order.
    dirty: Vec<InstanceId>,
    handlers: HashMap<NodeId, NodeHandlers>,
    pub(crate) out: Vec<Command>,
    /// Lists with a staged `set_list`, applied at the next flush.
    pub(crate) staged: Vec<(InstanceId, usize)>,
    /// The flush window and its clock (see [`crate::batch`]).
    pub(crate) window: Duration,
    pub(crate) clock: Clock,
    pub(crate) pending_since: Option<Duration>,
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
            staged: Vec::new(),
            window: Duration::ZERO,
            clock: instant_clock(),
            pending_since: None,
        }
    }

    /// Mount an instance of `compiled` under the bridge node `parent`. Its
    /// creates and appends are queued for the next [`flush`](Self::flush).
    pub fn mount(&mut self, compiled: &Arc<Compiled>, parent: NodeId) -> InstanceId {
        let instance = self.mount_detached(compiled, parent, compiled.template.slots.clone());
        let inst = self.instances[instance].as_ref().expect("just mounted");
        for (ix, node) in inst.compiled.template.nodes.iter().enumerate() {
            if node.parent.is_none() {
                self.out.push(Command::Append {
                    parent,
                    child: inst.ids[ix],
                });
            }
        }
        self.note_pending();
        instance
    }

    /// Create an instance's nodes (and their internal appends) without
    /// placing its roots; the caller positions them.
    pub(crate) fn mount_detached(
        &mut self,
        compiled: &Arc<Compiled>,
        parent: NodeId,
        slots: Vec<Value>,
    ) -> InstanceId {
        let instance = self.instances.len();
        let t = &compiled.template;
        let ids: Vec<NodeId> = (0..t.nodes.len() as NodeId)
            .map(|i| self.next_id + i)
            .collect();
        self.next_id += t.nodes.len() as NodeId;
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
                self.handlers
                    .insert(ids[ix], (instance, node.handlers.clone()));
            }
            self.out.push(Command::Create {
                id: ids[ix],
                kind: node.kind,
                props,
                style,
                text,
            });
        }
        for (ix, node) in t.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                self.out.push(Command::Append {
                    parent: ids[usize::from(p)],
                    child: ids[ix],
                });
            }
        }

        let lists = t
            .blocks
            .iter()
            .map(|Block::For { parent, item }| {
                ListState::new(ids[usize::from(*parent)], Arc::clone(item))
            })
            .collect();
        let n_bindings = t.bindings.len();
        self.instances.push(Some(Instance {
            compiled: Arc::clone(compiled),
            parent,
            ids,
            slots,
            sent,
            dirty: Vec::new(),
            is_dirty: vec![false; n_bindings],
            lists,
        }));
        instance
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
        let first = inst.dirty.is_empty();
        for &b in &inst.compiled.slot_deps[usize::from(slot)] {
            if !inst.is_dirty[usize::from(b)] {
                inst.is_dirty[usize::from(b)] = true;
                inst.dirty.push(b);
            }
        }
        if first && !inst.dirty.is_empty() {
            self.dirty.push(instance);
        }
        self.note_pending();
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
    /// the last flush (mounts, list changes, unmounts and changed values), in
    /// order.
    pub fn flush(&mut self) -> Vec<Command> {
        // Staged lists first: their field changes mark bindings the loop
        // below then evaluates.
        self.apply_staged();
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
        self.pending_since = None;
        std::mem::take(&mut self.out)
    }

    /// Remove an instance: its roots leave the mount parent (Bevy despawns the
    /// subtree, list items included), its handlers are forgotten.
    pub fn unmount(&mut self, instance: InstanceId) {
        let Some(Some(inst)) = self.instances.get(instance) else {
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
        self.forget(instance);
        self.note_pending();
    }

    /// Drop an instance and everything mounted inside it, emitting nothing
    /// (its nodes die with whatever removal the caller sent).
    pub(crate) fn forget(&mut self, instance: InstanceId) {
        let Some(inst) = self.instances.get_mut(instance).and_then(Option::take) else {
            return;
        };
        for id in &inst.ids {
            self.handlers.remove(id);
        }
        self.dirty.retain(|&i| i != instance);
        for list in inst.lists {
            for item in list.items.into_values() {
                self.forget(item);
            }
        }
    }

    /// The JS function to run for `event` on bridge node `node`, and the
    /// instance it belongs to (so a row's handler can read its row).
    pub fn handler(&self, node: NodeId, event: &str) -> Option<(JsRef, InstanceId)> {
        let (instance, handlers) = self.handlers.get(&node)?;
        handlers
            .iter()
            .find(|(ev, _)| *ev == event)
            .map(|(_, js)| (*js, *instance))
    }

    /// The bridge id of template node `ix` in `instance`.
    pub fn node_id(&self, instance: InstanceId, ix: usize) -> Option<NodeId> {
        self.instances.get(instance)?.as_ref()?.ids.get(ix).copied()
    }

    /// How many instances are alive (list items included).
    pub fn live_instances(&self) -> usize {
        self.instances.iter().filter(|i| i.is_some()).count()
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
