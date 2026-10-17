//! A prepared template: validated once, with the indexes mounting and
//! flushing need (which bindings read which slot, which selectors watch it).

use std::sync::Arc;

use crate::ir::{Block, Target, Template};

/// A template prepared for mounting: validated, with its slot → binding
/// index precomputed (the compiler knows every read statically).
#[derive(Debug)]
pub struct Compiled {
    pub(crate) template: Template,
    /// `slot_deps[slot]` = the bindings that read `slot`.
    pub(crate) slot_deps: Vec<Vec<u16>>,
    /// `selector_index[slot]` = the `(block, selector)` pairs selecting on `slot`.
    pub(crate) selector_index: Vec<Vec<(usize, usize)>>,
    /// Per node, the handler props as they cross the wire (`"onClick"`).
    pub(crate) handler_props: Vec<Vec<&'static str>>,
    /// Per node, the bindings that write it.
    pub(crate) node_bindings: Vec<Vec<u16>>,
    /// Does any node carry a handler?
    pub(crate) has_handlers: bool,
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
        let mut selector_index = vec![Vec::new(); template.slots.len()];
        for (
            block,
            Block::For {
                parent,
                item,
                selectors,
            },
        ) in template.blocks.iter().enumerate()
        {
            assert!(usize::from(*parent) < nodes.len(), "list parent {parent}");
            let item_slots = item.template.slots.len();
            assert!(
                item_slots >= selectors.len(),
                "list item has fewer slots than selectors"
            );
            let data = item_slots - selectors.len();
            for (i, sel) in selectors.iter().enumerate() {
                assert!(
                    usize::from(sel.field) < data,
                    "selector field {} is not a data slot",
                    sel.field
                );
                assert_eq!(
                    usize::from(sel.flag),
                    data + i,
                    "selector {i}: flag slots are the item's last slots, in order"
                );
                selector_index
                    .get_mut(usize::from(sel.from))
                    .unwrap_or_else(|| {
                        panic!(
                            "selector reads parent slot {}, which is not declared",
                            sel.from
                        )
                    })
                    .push((block, i));
            }
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
        // Child blocks depend on slots through ids after the bindings'.
        let n_bindings = template.bindings.len();
        for (c, block) in template.children.iter().enumerate() {
            let parent = usize::from(block.parent);
            assert!(parent < nodes.len(), "child block {c}: parent {parent}");
            assert!(
                !template
                    .blocks
                    .iter()
                    .any(|Block::For { parent: p, .. }| *p == block.parent),
                "child block {c}: node {parent} is a list's parent"
            );
            if let Some(before) = block.before {
                assert_eq!(
                    nodes.get(usize::from(before)).and_then(|n| n.parent),
                    Some(block.parent),
                    "child block {c}: node {before} is not a child of node {parent}"
                );
            }
            let child = &block.component.template;
            assert!(
                child.nodes.first().is_some_and(|n| n.parent.is_none())
                    && child.nodes.iter().filter(|n| n.parent.is_none()).count() == 1,
                "child block {c}: the component needs one root, node 0"
            );
            for (slot, _) in &block.props {
                assert!(
                    usize::from(*slot) < child.slots.len(),
                    "child block {c}: prop slot {slot} is not a child slot"
                );
            }
            let mut reads = block
                .when
                .iter()
                .flat_map(|w| w.reads())
                .collect::<Vec<_>>();
            reads.extend(block.props.iter().flat_map(|(_, e)| e.reads()));
            reads.sort_unstable();
            reads.dedup();
            for slot in reads {
                slot_deps
                    .get_mut(usize::from(slot))
                    .unwrap_or_else(|| {
                        panic!("child block {c} reads slot {slot}, which is not declared")
                    })
                    .push((n_bindings + c) as u16);
            }
        }
        let handler_props = nodes
            .iter()
            .map(|n| {
                n.handlers
                    .iter()
                    // Leaked once per template: bounded by the templates.
                    .map(|(ev, _)| &*Box::leak(handler_prop(ev).into_boxed_str()))
                    .collect()
            })
            .collect();
        let mut node_bindings = vec![Vec::new(); nodes.len()];
        for (b, binding) in template.bindings.iter().enumerate() {
            let (Target::Text(n) | Target::Prop(n, _) | Target::Style(n, _)) = binding.target;
            node_bindings[usize::from(n)].push(b as u16);
        }
        let has_handlers = nodes.iter().any(|n| !n.handlers.is_empty());
        Arc::new(Compiled {
            template,
            slot_deps,
            selector_index,
            handler_props,
            node_bindings,
            has_handlers,
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
