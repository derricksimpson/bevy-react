//! The view runtime: mounts compiled templates, holds their state, and turns
//! state writes into the minimal command stream.
//!
//! Writes are batched: [`Runtime::set`] only marks the bindings that read the
//! slot, and [`Runtime::flush`] re-evaluates each marked binding once and
//! emits a command only when its value differs from the last one sent. So a
//! burst of writes costs one evaluation per affected hole, and a write that
//! ends where it started emits nothing. Keyed lists are in [`crate::list`].

use std::sync::Arc;
use std::time::Duration;

use crate::batch::{Clock, instant_clock};
use crate::bits::Bits;
use crate::command::{Command, NodeId};
use crate::compiled::Compiled;
use crate::eval::eval;
use crate::ir::{Block, JsRef, SlotId, Target};
use crate::list::ListState;
use crate::value::Value;

/// A mounted component instance.
pub type InstanceId = usize;

pub(crate) struct Instance {
    pub(crate) compiled: Arc<Compiled>,
    pub(crate) parent: NodeId,
    /// The bridge id of template node 0; node `i` is `id0 + i` (ids are
    /// allocated per instance in one run, so there is no id table).
    pub(crate) id0: NodeId,
    pub(crate) slots: Vec<Value>,
    /// The value each binding last sent.
    sent: Vec<Value>,
    /// Bindings (then child blocks) marked by a write since the last flush.
    dirty: Vec<u16>,
    is_dirty: Bits,
    /// One per `Block::For`, in template order.
    pub(crate) lists: Vec<ListState>,
    /// One per `ChildBlock`: its mounted instance, if any.
    pub(crate) children: Vec<Option<InstanceId>>,
}

impl Instance {
    /// The bridge id of template node `ix`.
    pub(crate) fn node(&self, ix: usize) -> NodeId {
        self.id0 + ix as NodeId
    }
}

/// The runtime for one island tree.
pub struct Runtime {
    next_id: NodeId,
    pub(crate) instances: Vec<Option<Instance>>,
    /// Instances with marked bindings, in first-write order (entries for
    /// instances dropped since are skipped at flush).
    dirty: Vec<InstanceId>,
    /// `(id0, instance)` of every instance whose template has handlers, in
    /// ascending `id0` (ids are allocated upward), so an event's node id finds
    /// its instance by binary search. Entries of dropped instances linger
    /// until [`Runtime::compact_ranges`] sweeps them.
    ranges: Vec<(NodeId, InstanceId)>,
    dead_ranges: usize,
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
            ranges: Vec::new(),
            dead_ranges: 0,
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
                    child: inst.node(ix),
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
        let id0 = self.next_id;
        self.next_id += t.nodes.len() as NodeId;
        let sent: Vec<Value> = t.bindings.iter().map(|b| eval(&b.expr, &slots)).collect();

        // Creates carry the statics plus every binding's first value.
        for (ix, node) in t.nodes.iter().enumerate() {
            let mut props: Vec<(&'static str, Value)> = node.props.clone();
            props.extend(
                compiled.handler_props[ix]
                    .iter()
                    .map(|p| (*p, Value::Bool(true))),
            );
            let mut style = node.style.clone();
            let mut text = node.text.map(Arc::from);
            for &b in &compiled.node_bindings[ix] {
                let v = sent[usize::from(b)].clone();
                match t.bindings[usize::from(b)].target {
                    Target::Text(_) => text = Some(Arc::from(v.to_string())),
                    Target::Prop(_, key) => upsert(&mut props, key, v),
                    Target::Style(_, key) => upsert(&mut style, key, v),
                }
            }
            self.out.push(Command::Create {
                id: id0 + ix as NodeId,
                kind: node.kind,
                props,
                style,
                text,
            });
        }
        for (ix, node) in t.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                self.out.push(Command::Append {
                    parent: id0 + NodeId::from(p),
                    child: id0 + ix as NodeId,
                });
            }
        }
        if compiled.has_handlers {
            self.ranges.push((id0, instance));
        }

        let lists = t
            .blocks
            .iter()
            .map(
                |Block::For {
                     parent,
                     item,
                     selectors,
                 }| {
                    ListState::new(
                        id0 + NodeId::from(*parent),
                        Arc::clone(item),
                        selectors.clone(),
                    )
                },
            )
            .collect();
        let n_children = t.children.len();
        self.instances.push(Some(Instance {
            compiled: Arc::clone(compiled),
            parent,
            id0,
            slots,
            sent,
            dirty: Vec::new(),
            is_dirty: Bits::default(),
            lists,
            children: vec![None; n_children],
        }));
        for c in 0..n_children {
            self.sync_child(instance, c);
        }
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
        let selects = !inst.compiled.selector_index[usize::from(slot)].is_empty();
        let old = std::mem::replace(cur, value);
        let new = selects.then(|| cur.clone());
        let first = inst.dirty.is_empty();
        for &b in &inst.compiled.slot_deps[usize::from(slot)] {
            if !inst.is_dirty.set(usize::from(b)) {
                inst.dirty.push(b);
            }
        }
        if first && !inst.dirty.is_empty() {
            self.dirty.push(instance);
        }
        self.note_pending();
        if let Some(new) = new {
            self.retarget(instance, slot, old, new);
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
    /// the last flush (mounts, list changes, unmounts and changed values), in
    /// order.
    pub fn flush(&mut self) -> Vec<Command> {
        // Staged lists first: their field changes mark bindings the loop
        // below then evaluates.
        self.apply_staged();
        // Mounting a child or pushing a prop marks more bindings: go round
        // until nothing is left (parents only ever mark their children).
        while !self.dirty.is_empty() {
            for instance in std::mem::take(&mut self.dirty) {
                let Some(Some(inst)) = self.instances.get_mut(instance) else {
                    continue;
                };
                let n_bindings = inst.compiled.template.bindings.len();
                let mut marked = std::mem::take(&mut inst.dirty);
                // Template order, not write order: a deterministic stream.
                marked.sort_unstable();
                let mut blocks = Vec::new();
                for b in marked {
                    inst.is_dirty.clear(usize::from(b));
                    if usize::from(b) >= n_bindings {
                        blocks.push(usize::from(b) - n_bindings);
                        continue;
                    }
                    let binding = &inst.compiled.template.bindings[usize::from(b)];
                    let v = eval(&binding.expr, &inst.slots);
                    if v == inst.sent[usize::from(b)] {
                        continue; // changed and changed back, or an equal result
                    }
                    let cmd = match binding.target {
                        Target::Text(n) => Command::UpdateText {
                            id: inst.node(usize::from(n)),
                            text: Arc::from(v.to_string()),
                        },
                        Target::Prop(n, key) => Command::SetProp {
                            id: inst.node(usize::from(n)),
                            key,
                            value: v.clone(),
                        },
                        Target::Style(n, key) => Command::SetStyle {
                            id: inst.node(usize::from(n)),
                            key,
                            value: v.clone(),
                        },
                    };
                    inst.sent[usize::from(b)] = v;
                    self.out.push(cmd);
                }
                for c in blocks {
                    self.sync_child(instance, c);
                }
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
                    child: inst.node(ix),
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
        if inst.compiled.has_handlers {
            self.dead_ranges += 1;
            self.compact_ranges();
        }
        for list in inst.lists {
            for item in list.items.into_values() {
                self.forget(item);
            }
        }
        for child in inst.children.into_iter().flatten() {
            self.forget(child);
        }
    }

    /// The instance a bridge node id belongs to, if it is alive and its
    /// template has handlers.
    fn instance_of(&self, node: NodeId) -> Option<(InstanceId, &Instance)> {
        let at = self.ranges.partition_point(|(id0, _)| *id0 <= node);
        let &(id0, instance) = self.ranges.get(at.checked_sub(1)?)?;
        let inst = self.instances.get(instance)?.as_ref()?;
        // A stale range (its instance dropped) fails the id0 check.
        (inst.id0 == id0 && ((node - id0) as usize) < inst.compiled.template.nodes.len())
            .then_some((instance, inst))
    }

    /// Sweep ranges of dropped instances once they are most of the table.
    fn compact_ranges(&mut self) {
        if self.dead_ranges < 1024 || self.dead_ranges * 2 < self.ranges.len() {
            return;
        }
        let instances = &self.instances;
        self.ranges
            .retain(|&(id0, i)| instances[i].as_ref().is_some_and(|inst| inst.id0 == id0));
        self.dead_ranges = 0;
    }

    /// The JS function to run for `event` on bridge node `node`, and the
    /// instance it belongs to (so a row's handler can read its row).
    pub fn handler(&self, node: NodeId, event: &str) -> Option<(JsRef, InstanceId)> {
        let (instance, inst) = self.instance_of(node)?;
        inst.compiled.template.nodes[(node - inst.id0) as usize]
            .handlers
            .iter()
            .find(|(ev, _)| *ev == event)
            .map(|(_, js)| (*js, instance))
    }

    /// The bridge id of template node `ix` in `instance`.
    pub fn node_id(&self, instance: InstanceId, ix: usize) -> Option<NodeId> {
        let inst = self.instances.get(instance)?.as_ref()?;
        (ix < inst.compiled.template.nodes.len()).then(|| inst.node(ix))
    }

    /// How many instances are alive (list items included).
    pub fn live_instances(&self) -> usize {
        self.instances.iter().filter(|i| i.is_some()).count()
    }
}

/// Set `key` in a small `(key, value)` list, replacing an earlier entry.
fn upsert(list: &mut Vec<(&'static str, Value)>, key: &'static str, v: Value) {
    match list.iter_mut().find(|(k, _)| *k == key) {
        Some(slot) => slot.1 = v,
        None => list.push((key, v)),
    }
}
