//! Keyed lists: [`Runtime::set_list`] applies a new list to a `Block::For`.
//!
//! The JS side owns the data. It sends the new key order with the fields of
//! each entry that is new or whose record changed (`fields: Some`); an entry
//! whose record is the same object as last time sends `fields: None`, so an
//! untouched row costs one key on the wire and nothing in Rust. Positions go
//! through [`keyed::diff`](crate::keyed::diff); field changes go through the
//! item instance's slots, so they ride the next flush like any state write.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::command::{Command, NodeId};
use crate::keyed::{self, Key, Step};
use crate::runtime::{Compiled, InstanceId, Runtime};
use crate::value::Value;

/// One entry of a new list.
#[derive(Clone, Debug)]
pub struct ListItem {
    pub key: Key,
    /// The entry's fields, in the item template's slot order; `None` = the
    /// record did not change (the key must already be in the list).
    pub fields: Option<Vec<Value>>,
}

impl ListItem {
    pub fn new(key: impl Into<Key>, fields: Vec<Value>) -> Self {
        ListItem {
            key: key.into(),
            fields: Some(fields),
        }
    }

    pub fn same(key: impl Into<Key>) -> Self {
        ListItem {
            key: key.into(),
            fields: None,
        }
    }
}

/// Why a list was refused (nothing is applied).
#[derive(Clone, Debug, PartialEq)]
pub enum ListError {
    NoSuchList,
    DuplicateKey(Key),
    /// A new key sent without its fields.
    MissingFields(Key),
    WrongFieldCount {
        key: Key,
        expected: usize,
        got: usize,
    },
}

/// A mounted list.
pub(crate) struct ListState {
    /// The node whose children the items are.
    parent: NodeId,
    item: Arc<Compiled>,
    keys: Vec<Key>,
    pub(crate) items: HashMap<Key, InstanceId>,
}

impl ListState {
    pub(crate) fn new(parent: NodeId, item: Arc<Compiled>) -> Self {
        ListState {
            parent,
            item,
            keys: Vec::new(),
            items: HashMap::new(),
        }
    }
}

impl Runtime {
    /// Replace list `block` of `instance` with `items`. Structural commands
    /// (creates, removes, moves) are queued now; field changes of kept items
    /// ride the next [`flush`](Self::flush).
    pub fn set_list(
        &mut self,
        instance: InstanceId,
        block: usize,
        items: Vec<ListItem>,
    ) -> Result<(), ListError> {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return Err(ListError::NoSuchList);
        };
        let Some(list) = inst.lists.get_mut(block) else {
            return Err(ListError::NoSuchList);
        };
        // Validate everything before touching anything.
        let expected = list.item.template().slots.len();
        let mut seen = HashSet::with_capacity(items.len());
        for it in &items {
            if !seen.insert(&it.key) {
                return Err(ListError::DuplicateKey(it.key.clone()));
            }
            match &it.fields {
                None if !list.items.contains_key(&it.key) => {
                    return Err(ListError::MissingFields(it.key.clone()));
                }
                Some(f) if f.len() != expected => {
                    return Err(ListError::WrongFieldCount {
                        key: it.key.clone(),
                        expected,
                        got: f.len(),
                    });
                }
                _ => {}
            }
        }
        // Work on the list out of its instance (mounting borrows `self`).
        let mut list = std::mem::replace(list, ListState::new(0, Arc::clone(&list.item)));
        let new_keys: Vec<Key> = items.iter().map(|it| it.key.clone()).collect();
        let d = keyed::diff(&list.keys, &new_keys);

        for &o in &d.removed {
            let item = list
                .items
                .remove(&list.keys[o])
                .expect("old key is mounted");
            self.out.push(Command::Remove {
                parent: list.parent,
                child: self.root(item),
            });
            self.forget(item);
        }
        let mut fresh: HashMap<usize, Vec<Value>> = HashMap::new();
        for (i, it) in items.into_iter().enumerate() {
            let Some(fields) = it.fields else { continue };
            match list.items.get(&it.key) {
                Some(&item) => fields
                    .into_iter()
                    .enumerate()
                    .for_each(|(s, v)| self.set(item, s as u16, v)),
                None => {
                    fresh.insert(i, fields);
                }
            }
        }
        for step in &d.steps {
            let (new, before) = match *step {
                Step::Insert { new, before } => {
                    let slots = fresh
                        .remove(&new)
                        .expect("validated: new keys carry fields");
                    let item = self.mount_detached(&list.item, list.parent, slots);
                    list.items.insert(new_keys[new].clone(), item);
                    (new, before)
                }
                Step::Move { new, before, .. } => (new, before),
            };
            let child = self.root(list.items[&new_keys[new]]);
            self.out.push(match before {
                Some(b) => Command::Insert {
                    parent: list.parent,
                    child,
                    before: self.root(list.items[&new_keys[b]]),
                },
                None => Command::Append {
                    parent: list.parent,
                    child,
                },
            });
        }
        list.keys = new_keys;
        if let Some(Some(inst)) = self.instances.get_mut(instance) {
            inst.lists[block] = list;
        }
        Ok(())
    }

    /// The bridge id of an item's root (node 0, checked by `Compiled::new`).
    fn root(&self, item: InstanceId) -> NodeId {
        self.instances[item].as_ref().expect("live item").ids[0]
    }

    /// The instance of list entry `key` (to read or write its fields).
    pub fn list_item(&self, instance: InstanceId, block: usize, key: &Key) -> Option<InstanceId> {
        self.instances
            .get(instance)?
            .as_ref()?
            .lists
            .get(block)?
            .items
            .get(key)
            .copied()
    }
}
