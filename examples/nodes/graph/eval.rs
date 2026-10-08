//! The scheduler: orders the graph, keys every node by its content, and
//! evaluates the nodes whose key isn't cached yet, upstream first, within a
//! per-frame time budget.
//!
//! A node's key hashes its kind, values, mute, the resolution, the outputs
//! anyone wants from it and its inputs' keys, so a change re-keys exactly the
//! node and everything downstream — and going back (undo, a slider dragged
//! back) finds the old results still cached.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy_react::ReactEvents;

use super::buf::Buf;
use super::catalog::{self, NodeDef};
use super::nodes::{self, Ctx};
use super::{SetGraph, Status, Timing, Value, ViewOutputState};

/// Time spent evaluating per frame. At least one node always runs, so a
/// slow node still makes progress.
const BUDGET_MS: f32 = 10.0;

/// Results kept beyond the current graph's (undo, slider round trips).
const SPARE_ENTRIES: usize = 48;

pub type Outputs = Arc<Vec<Option<Arc<Buf>>>>;

/// One node of the current graph, resolved against the catalog.
pub struct Slot {
    pub id: u32,
    pub def: &'static NodeDef,
    /// Per input socket (catalog order): the upstream slot and its output.
    pub inputs: Vec<Option<(usize, usize)>>,
    /// Bit `i` set: someone needs output `i`.
    pub wanted: u32,
    pub key: u64,
}

#[derive(Resource, Default)]
pub struct Evaluator {
    pub graph: SetGraph,
    pub slots: Vec<Slot>,
    /// Slot indices, upstream first. Nodes on a cycle are left out.
    order: Vec<usize>,
    cache: HashMap<u64, (Outputs, u64)>,
    /// Insertion counter, to evict the oldest spare entries.
    tick: u64,
    timings: HashMap<u32, f32>,
    /// The graph or the view changed: rebuild the slots.
    stale: bool,
    view: super::ViewOutput,
    last_status: Status,
}

impl Evaluator {
    pub fn set(&mut self, graph: SetGraph) {
        self.graph = graph;
        self.stale = true;
    }

    pub fn resolution(&self) -> usize {
        self.graph.resolution.clamp(16, 2048) as usize
    }

    pub fn slot(&self, id: u32) -> Option<&Slot> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// A node's outputs, once evaluated for its current key.
    pub fn outputs(&self, slot: &Slot) -> Option<&Outputs> {
        self.cache.get(&slot.key).map(|(o, _)| o)
    }

    /// The buffer an input socket reads, when linked and evaluated.
    pub fn input(&self, slot: &Slot, socket: usize) -> Option<Arc<Buf>> {
        let (up, output) = slot.inputs.get(socket).copied().flatten()?;
        self.outputs(&self.slots[up])?.get(output)?.clone()
    }

    fn rebuild(&mut self) {
        self.slots = self
            .graph
            .nodes
            .iter()
            .filter_map(|n| {
                let def = catalog::get(&n.kind)?;
                Some(Slot {
                    id: n.id,
                    def,
                    inputs: vec![None; def.inputs.len()],
                    wanted: 1,
                    key: 0,
                })
            })
            .collect();
        // `graph.nodes` and `slots` differ only by unknown kinds; map node
        // ids to slot indices.
        let slot_of: HashMap<u32, usize> = self
            .slots
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();
        for link in &self.graph.links {
            let (Some(&from), Some(&to)) = (slot_of.get(&link.from), slot_of.get(&link.to)) else {
                continue;
            };
            let Some(output) = self.slots[from]
                .def
                .outputs
                .iter()
                .position(|s| s.id == link.output)
            else {
                continue;
            };
            let Some(input) = self.slots[to]
                .def
                .inputs
                .iter()
                .position(|s| s.id == link.input)
            else {
                continue;
            };
            self.slots[to].inputs[input] = Some((from, output));
            self.slots[from].wanted |= 1 << output;
        }
        if let Some(node) = self.view.node
            && let Some(&i) = slot_of.get(&node)
            && let Some(o) = self
                .view
                .output
                .as_ref()
                .and_then(|out| self.slots[i].def.outputs.iter().position(|s| s.id == out))
        {
            self.slots[i].wanted |= 1 << o;
        }

        // Kahn's algorithm; whatever never reaches in-degree 0 is on a cycle.
        let mut indegree: Vec<usize> = self
            .slots
            .iter()
            .map(|s| s.inputs.iter().flatten().count())
            .collect();
        let mut ready: Vec<usize> = (0..self.slots.len())
            .filter(|&i| indegree[i] == 0)
            .collect();
        self.order.clear();
        while let Some(i) = ready.pop() {
            self.order.push(i);
            for (j, slot) in self.slots.iter().enumerate() {
                for _ in slot.inputs.iter().flatten().filter(|(up, _)| *up == i) {
                    indegree[j] -= 1;
                    if indegree[j] == 0 {
                        ready.push(j);
                    }
                }
            }
        }

        let values: HashMap<u32, &super::GraphNode> =
            self.graph.nodes.iter().map(|n| (n.id, n)).collect();
        for &i in &self.order {
            let mut h = DefaultHasher::new();
            let node = values[&self.slots[i].id];
            node.kind.hash(&mut h);
            for (k, v) in &node.values {
                k.hash(&mut h);
                hash_value(v, &mut h);
            }
            node.mute.hash(&mut h);
            self.graph.resolution.hash(&mut h);
            self.slots[i].wanted.hash(&mut h);
            for input in &self.slots[i].inputs {
                match input {
                    Some((up, output)) => (self.slots[*up].key, *output).hash(&mut h),
                    None => 0u64.hash(&mut h),
                }
            }
            self.slots[i].key = h.finish();
        }

        // Keep the current results plus the newest spare ones.
        let live: std::collections::HashSet<u64> = self.slots.iter().map(|s| s.key).collect();
        let mut spare: Vec<(u64, u64)> = self
            .cache
            .iter()
            .filter(|(k, _)| !live.contains(k))
            .map(|(k, (_, t))| (*k, *t))
            .collect();
        spare.sort_by_key(|&(_, t)| std::cmp::Reverse(t));
        for (k, _) in spare.into_iter().skip(SPARE_ENTRIES) {
            self.cache.remove(&k);
        }
        let ids: std::collections::HashSet<u32> = self.slots.iter().map(|s| s.id).collect();
        self.timings.retain(|id, _| ids.contains(id));
    }

    /// The nodes of the current graph still to evaluate.
    fn pending(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .order
            .iter()
            .filter(|&&i| !self.cache.contains_key(&self.slots[i].key))
            .map(|&i| self.slots[i].id)
            .collect();
        ids.sort_unstable();
        ids
    }

    fn step(&mut self, budget_ms: f32) {
        let start = Instant::now();
        let n = self.resolution();
        let values: HashMap<u32, &super::GraphNode> =
            self.graph.nodes.iter().map(|n| (n.id, n)).collect();
        let mut ran = false;
        for &i in &self.order {
            let slot = &self.slots[i];
            if self.cache.contains_key(&slot.key) {
                continue;
            }
            if ran && start.elapsed().as_secs_f32() * 1000.0 > budget_ms {
                break;
            }
            let inputs: Vec<Option<Arc<Buf>>> = (0..slot.inputs.len())
                .map(|socket| self.input(slot, socket))
                .collect();
            let node = values[&slot.id];
            let began = Instant::now();
            let outputs: Vec<Option<Arc<Buf>>> = if node.mute || slot.def.kind == "reroute" {
                // Pass the first linked input through every output.
                let through = inputs.iter().flatten().next().cloned();
                slot.def
                    .outputs
                    .iter()
                    .map(|_| {
                        through
                            .clone()
                            .or_else(|| Some(Arc::new(Buf::Uniform(Vec3::ZERO))))
                    })
                    .collect()
            } else {
                let ctx = Ctx {
                    n,
                    def: slot.def,
                    values: &node.values,
                    inputs: &inputs,
                    wanted: slot.wanted,
                };
                nodes::eval(&ctx)
                    .into_iter()
                    .map(|b| b.map(Arc::new))
                    .collect()
            };
            self.timings
                .insert(slot.id, began.elapsed().as_secs_f32() * 1000.0);
            self.tick += 1;
            self.cache.insert(slot.key, (Arc::new(outputs), self.tick));
            ran = true;
        }
    }
}

fn hash_value(v: &Value, h: &mut impl Hasher) {
    match v {
        Value::Bool(b) => (0u8, b).hash(h),
        Value::Num(x) => (1u8, x.to_bits()).hash(h),
        Value::Color(c) => (2u8, c.map(f32::to_bits)).hash(h),
        Value::Text(s) => (3u8, s).hash(h),
        Value::Ramp(stops) => {
            4u8.hash(h);
            for s in stops {
                (s.pos.to_bits(), s.color.map(f32::to_bits)).hash(h);
            }
        }
    }
}

pub fn evaluate(mut evaluator: ResMut<Evaluator>, view: Res<ViewOutputState>) {
    if evaluator.view != view.0 {
        evaluator.view = view.0.clone();
        evaluator.stale = true;
    }
    if evaluator.stale {
        evaluator.stale = false;
        evaluator.rebuild();
    }
    evaluator.step(BUDGET_MS);
}

/// Tell React how far evaluation is, when that changes.
pub fn report_status(mut evaluator: ResMut<Evaluator>, events: ReactEvents) {
    let mut timings: Vec<Timing> = evaluator
        .timings
        .iter()
        .map(|(&id, &ms)| Timing {
            id,
            ms: (ms * 10.0).round() / 10.0,
        })
        .collect();
    timings.sort_by_key(|t| t.id);
    let status = Status {
        pending: evaluator.pending(),
        timings,
    };
    if status != evaluator.last_status {
        events.send(&status);
        evaluator.last_status = status;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphLink, GraphNode};

    fn node(id: u32, kind: &str) -> GraphNode {
        GraphNode {
            id,
            kind: kind.into(),
            values: Default::default(),
            mute: false,
        }
    }

    fn link(from: u32, output: &str, to: u32, input: &str) -> GraphLink {
        GraphLink {
            from,
            output: output.into(),
            to,
            input: input.into(),
        }
    }

    /// Changing a node re-keys it and its downstream only; changing it back
    /// finds every result cached.
    #[test]
    fn keys_follow_content_and_results_stay_cached() {
        let mut ev = Evaluator::default();
        let graph = |scale: f32| SetGraph {
            resolution: 16,
            nodes: vec![
                GraphNode {
                    values: [("scale".to_string(), Value::Num(scale))].into(),
                    ..node(1, "noise")
                },
                node(2, "invert"),
                node(3, "bricks"),
            ],
            links: vec![link(1, "fac", 2, "color")],
        };
        ev.set(graph(4.0));
        ev.rebuild();
        ev.step(1000.0);
        assert!(ev.pending().is_empty());
        let keys: Vec<u64> = ev.slots.iter().map(|s| s.key).collect();

        ev.set(graph(5.0));
        ev.rebuild();
        assert_eq!(ev.pending(), vec![1, 2], "noise and its downstream invert");
        assert_eq!(ev.slots[2].key, keys[2], "bricks untouched");
        ev.step(1000.0);

        ev.set(graph(4.0));
        ev.rebuild();
        assert!(ev.pending().is_empty(), "back to cached results");
    }

    #[test]
    fn cycles_are_left_out() {
        let mut ev = Evaluator::default();
        ev.set(SetGraph {
            resolution: 16,
            nodes: vec![node(1, "invert"), node(2, "invert")],
            links: vec![link(1, "color", 2, "color"), link(2, "color", 1, "color")],
        });
        ev.rebuild();
        ev.step(1000.0);
        assert_eq!(ev.order.len(), 0);
    }
}
