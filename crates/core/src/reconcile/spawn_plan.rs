//! Spawn-time parenting: a pre-pass over one batch of ops that finds the fresh
//! nodes which can be spawned with their `ChildOf` already in the bundle.
//!
//! The generic path creates every node parentless and attaches it on its
//! `Append`, i.e. inserts `ChildOf` afterwards — one archetype move per node,
//! which for a mass create is most of the apply (`.scratch/compiled-views`
//! has the profile). Spawning the child with `ChildOf(parent)` skips the move
//! and builds the same `Children` list, provided the spawn order of a parent's
//! children equals their `Append` order. That is checked here, per parent:
//!
//! - a **pure** parent only receives `Append`s of nodes created in this batch,
//!   each appended once, in the order they were created. Any `Insert`/`Remove`
//!   under it, a moved or re-appended child, or an `Append` out of creation
//!   order makes it impure, and its children take the generic path;
//! - a parent created *after* its children (React builds bottom-up) has its
//!   entity reserved when the first child spawns ([`SpawnPlan::parent_entity`])
//!   and filled in at its own `Create` ([`SpawnPlan::take_reserved`]);
//! - a batch with a `Reset` is not planned at all (the id maps are cleared
//!   mid-batch), nor one whose created ids are too sparse for the dense tables
//!   below (ids are a counter, so a batch's are normally one contiguous run).
//!
//! The `Append` arm still runs for every planned child (shadow tree, layer
//! dirt, text-style inheritance); it only skips the ECS attach for the ids in
//! [`SpawnPlan::attached`].

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::bridge::JsBridge;
use crate::protocol::{NodeId, op::Op};

/// "No parent" in the dense parent table.
const NONE: NodeId = NodeId::MAX;

/// Dense tables over the batch's created ids `lo..lo + len` pay for a hash
/// map's worth of work per op with an index; give up when the span is wider
/// than this many slots per created node (plus slack).
const SPARSE_SLOTS_PER_NODE: usize = 8;
const SPARSE_SLACK: usize = 1024;

/// What the pre-pass knows of one parent.
#[derive(Clone, Copy, Default)]
struct ParentState {
    /// Creation sequence number (1-based) of its last appended child.
    last: u32,
    impure: bool,
}

#[derive(Default)]
pub(super) struct SpawnPlan {
    lo: NodeId,
    /// Fresh child → the parent it is appended to, for pure parents only.
    parent: Vec<NodeId>,
    /// Parents created later in the batch whose entity a child already needed.
    reserved: Vec<Option<Entity>>,
    /// Children spawned with their `ChildOf`: the `Append` arm skips the attach.
    attached: Vec<bool>,
}

impl SpawnPlan {
    fn slot(&self, id: NodeId) -> Option<usize> {
        let i = id.checked_sub(self.lo)? as usize;
        (i < self.parent.len()).then_some(i)
    }

    pub(super) fn build(ops: &[Op], bridge: &JsBridge) -> Self {
        let (mut lo, mut hi, mut n) = (NodeId::MAX, 0, 0usize);
        for op in ops {
            match op {
                Op::Reset => return Self::default(),
                Op::Create { id, .. }
                | Op::CreateText { id, .. }
                | Op::CreateTextSpan { id, .. } => {
                    (lo, hi, n) = (lo.min(*id), hi.max(*id), n + 1);
                }
                _ => {}
            }
        }
        if n == 0 {
            return Self::default();
        }
        let len = (hi - lo) as usize + 1;
        if len > n * SPARSE_SLOTS_PER_NODE + SPARSE_SLACK {
            return Self::default();
        }
        let mut plan = Self {
            lo,
            parent: vec![NONE; len],
            reserved: vec![None; len],
            attached: vec![false; len],
        };
        // Creation sequence numbers (1-based; 0 = not created in this batch).
        let mut seq = vec![0u32; len];
        let mut state = vec![ParentState::default(); len];
        // Parents that existed before the batch.
        let mut outside: HashMap<NodeId, ParentState> = HashMap::new();
        let mut next = 0u32;
        // A parent's state, wherever its id falls.
        macro_rules! state_of {
            ($plan:expr, $p:expr) => {
                match $plan.slot($p) {
                    Some(i) => &mut state[i],
                    None => outside.entry($p).or_default(),
                }
            };
        }
        for op in ops {
            match op {
                Op::Create { id, .. }
                | Op::CreateText { id, .. }
                | Op::CreateTextSpan { id, .. } => {
                    next += 1;
                    seq[(*id - lo) as usize] = next;
                }
                Op::Append { parent, child } => {
                    let child_slot = plan.slot(*child).filter(|&i| seq[i] != 0);
                    let parent_known = match plan.slot(*parent) {
                        Some(i) => seq[i] != 0,
                        None => bridge.nodes.contains_key(parent),
                    };
                    match child_slot {
                        Some(c) if plan.parent[c] == NONE && parent_known => {
                            let ps = state_of!(plan, *parent);
                            if seq[c] <= ps.last {
                                ps.impure = true;
                            } else {
                                ps.last = seq[c];
                                plan.parent[c] = *parent;
                            }
                        }
                        _ => {
                            state_of!(plan, *parent).impure = true;
                            if let Some(c) = child_slot
                                && plan.parent[c] != NONE
                            {
                                state_of!(plan, plan.parent[c]).impure = true;
                            }
                        }
                    }
                }
                Op::Insert { parent, child, .. } | Op::Remove { parent, child } => {
                    state_of!(plan, *parent).impure = true;
                    if let Some(c) = plan.slot(*child)
                        && plan.parent[c] != NONE
                    {
                        state_of!(plan, plan.parent[c]).impure = true;
                    }
                }
                Op::Update { .. } | Op::UpdateText { .. } | Op::Reset => {}
            }
        }
        for i in 0..len {
            let p = plan.parent[i];
            if p == NONE {
                continue;
            }
            let impure = match plan.slot(p) {
                Some(j) => state[j].impure,
                None => outside[&p].impure,
            };
            if impure {
                plan.parent[i] = NONE;
            }
        }
        plan
    }

    /// The entity to parent `child` under at spawn, reserving it (queued
    /// before the child's own spawn) when the parent is created later.
    pub(super) fn parent_entity(
        &mut self,
        commands: &mut Commands,
        bridge: &JsBridge,
        child: NodeId,
    ) -> Option<Entity> {
        let parent = self
            .slot(child)
            .map(|i| self.parent[i])
            .filter(|&p| p != NONE)?;
        if let Some(&e) = bridge.nodes.get(&parent) {
            return Some(e);
        }
        // Not mounted yet, so created in this batch (the build checked).
        let i = self.slot(parent)?;
        Some(*self.reserved[i].get_or_insert_with(|| commands.spawn_empty().id()))
    }

    /// The entity reserved for `id` by a child that spawned before it.
    pub(super) fn take_reserved(&mut self, id: NodeId) -> Option<Entity> {
        self.slot(id).and_then(|i| self.reserved[i].take())
    }

    /// Record that `id` was spawned under its planned parent.
    pub(super) fn mark_attached(&mut self, id: NodeId) {
        if let Some(i) = self.slot(id) {
            self.attached[i] = true;
        }
    }

    /// Whether `id` already has its `ChildOf` (the `Append` skips the attach).
    pub(super) fn is_attached(&self, id: NodeId) -> bool {
        self.slot(id).is_some_and(|i| self.attached[i])
    }

    /// How many fresh nodes the plan spawns parented.
    #[cfg(test)]
    pub(super) fn planned(&self) -> usize {
        self.parent.iter().filter(|&&p| p != NONE).count()
    }
}
