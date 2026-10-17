//! Test helpers shared by the integration tests.
#![allow(dead_code)]

use std::collections::HashMap;

use bevy_react_views::{Command, NodeId};

/// A model of the bridge's hierarchy: what `apply_js_ops` would build.
#[derive(Default)]
pub struct Tree {
    pub children: HashMap<NodeId, Vec<NodeId>>,
    pub text: HashMap<NodeId, String>,
    pub alive: HashMap<NodeId, &'static str>,
}

impl Tree {
    pub fn apply(&mut self, cmds: &[Command]) {
        for c in cmds {
            match c {
                Command::Create { id, kind, text, .. } => {
                    assert!(self.alive.insert(*id, kind).is_none(), "{id} created twice");
                    if let Some(t) = text {
                        self.text.insert(*id, t.to_string());
                    }
                }
                Command::Append { parent, child } => {
                    self.detach(*child);
                    self.children.entry(*parent).or_default().push(*child);
                }
                Command::Insert {
                    parent,
                    child,
                    before,
                } => {
                    self.detach(*child);
                    let kids = self.children.entry(*parent).or_default();
                    let at = kids
                        .iter()
                        .position(|k| k == before)
                        .expect("anchor is a child");
                    kids.insert(at, *child);
                }
                Command::Remove { parent, child } => {
                    let kids = self.children.get_mut(parent).expect("parent has children");
                    kids.retain(|k| k != child);
                    self.despawn(*child);
                }
                Command::UpdateText { id, text } => {
                    assert!(self.alive.contains_key(id), "text of a dead node {id}");
                    self.text.insert(*id, text.to_string());
                }
                Command::SetProp { .. } | Command::SetStyle { .. } => {}
            }
        }
    }

    fn detach(&mut self, child: NodeId) {
        for kids in self.children.values_mut() {
            kids.retain(|k| *k != child);
        }
    }

    fn despawn(&mut self, id: NodeId) {
        self.alive.remove(&id);
        self.text.remove(&id);
        for k in self.children.remove(&id).unwrap_or_default() {
            self.despawn(k);
        }
    }

    /// The labels of the table's rows, in order.
    pub fn rows(&self, table: NodeId) -> Vec<String> {
        self.children
            .get(&table)
            .map(|rows| {
                rows.iter()
                    .map(|r| self.text[&self.children[r][0]].clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}
