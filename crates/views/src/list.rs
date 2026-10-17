//! Keyed lists: [`Runtime::set_list`] stages a new list for a `Block::For`.
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
use crate::compiled::Compiled;
use crate::ir::Selector;
use crate::keyed::{self, Key, Step};
use crate::runtime::{InstanceId, Runtime};
use crate::select;
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
    pub(crate) selectors: Vec<Selector>,
    pub(crate) items: HashMap<Key, InstanceId>,
    /// A list staged by [`Runtime::set_list`], applied at the next flush. Its
    /// `fields: None` entries are unchanged relative to the **applied** list.
    pending: Option<Vec<ListItem>>,
}

impl ListState {
    pub(crate) fn new(parent: NodeId, item: Arc<Compiled>, selectors: Vec<Selector>) -> Self {
        ListState {
            parent,
            item,
            keys: Vec::new(),
            selectors,
            items: HashMap::new(),
            pending: None,
        }
    }

    /// How many slots an entry's fields fill (the flag slots follow).
    fn data_slots(&self) -> usize {
        self.item.template().slots.len() - self.selectors.len()
    }

    /// Validate `items` and stage them (merged over a list already staged).
    /// Returns whether the list was newly staged. Nothing changes on error.
    fn stage(&mut self, mut items: Vec<ListItem>) -> Result<bool, ListError> {
        let expected = self.data_slots();
        let wrong_count = |it: &ListItem| match &it.fields {
            Some(f) if f.len() != expected => Some(ListError::WrongFieldCount {
                key: it.key.clone(),
                expected,
                got: f.len(),
            }),
            _ => None,
        };
        match self.pending.as_mut() {
            None => {
                // Same keys in the same order as the applied list: already
                // unique and all known, so no hashing.
                let same_order = same_keys(&self.keys, &items);
                if !same_order {
                    let mut seen = HashSet::with_capacity(items.len());
                    for it in &items {
                        if !seen.insert(&it.key) {
                            return Err(ListError::DuplicateKey(it.key.clone()));
                        }
                    }
                }
                for it in &items {
                    if let Some(e) = wrong_count(it) {
                        return Err(e);
                    }
                    if !same_order && it.fields.is_none() && !self.items.contains_key(&it.key) {
                        return Err(ListError::MissingFields(it.key.clone()));
                    }
                }
                self.pending = Some(items);
                Ok(true)
            }
            Some(prev) => {
                // An entry sent as unchanged is relative to the staged list;
                // fold it back to be relative to the applied one.
                if same_keys_of(prev, &items) {
                    for it in &items {
                        if let Some(e) = wrong_count(it) {
                            return Err(e);
                        }
                    }
                    for (it, p) in items.iter_mut().zip(prev.iter_mut()) {
                        if let Some(f) = it.fields.take() {
                            p.fields = Some(f);
                        }
                    }
                    return Ok(false); // `prev` now holds the merge
                }
                let mut seen = HashSet::with_capacity(items.len());
                let before: HashMap<&Key, &Option<Vec<Value>>> =
                    prev.iter().map(|p| (&p.key, &p.fields)).collect();
                for it in &items {
                    if !seen.insert(&it.key) {
                        return Err(ListError::DuplicateKey(it.key.clone()));
                    }
                    if let Some(e) = wrong_count(it) {
                        return Err(e);
                    }
                    if it.fields.is_none() && !before.contains_key(&it.key) {
                        return Err(ListError::MissingFields(it.key.clone()));
                    }
                }
                for it in &mut items {
                    if it.fields.is_none() {
                        it.fields = before[&it.key].clone();
                    }
                }
                drop(before);
                self.pending = Some(items);
                Ok(false)
            }
        }
    }
}

fn same_keys(keys: &[Key], items: &[ListItem]) -> bool {
    keys.len() == items.len() && keys.iter().zip(items).all(|(k, it)| *k == it.key)
}

fn same_keys_of(a: &[ListItem], b: &[ListItem]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.key == y.key)
}

impl Runtime {
    /// Stage `items` as the new content of list `block` of `instance`. Nothing
    /// is applied until the next [`flush`](Self::flush); lists staged before
    /// it coalesce into one diff against what is applied. A refused list
    /// changes nothing.
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
        if list.stage(items)? {
            self.staged.push((instance, block));
        }
        self.note_pending();
        Ok(())
    }

    /// Change the fields of one existing entry, touching nothing else: the
    /// delta path for a record that changed in place (no list-wide work). If
    /// the list has a staged `set_list`, the edit folds into the staged entry,
    /// so a row about to be created is created with its final fields.
    pub fn update_item(
        &mut self,
        instance: InstanceId,
        block: usize,
        key: &Key,
        fields: Vec<Value>,
    ) -> Result<(), ListError> {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return Err(ListError::NoSuchList);
        };
        let Some(list) = inst.lists.get_mut(block) else {
            return Err(ListError::NoSuchList);
        };
        let expected = list.data_slots();
        if fields.len() != expected {
            return Err(ListError::WrongFieldCount {
                key: key.clone(),
                expected,
                got: fields.len(),
            });
        }
        if let Some(pending) = list.pending.as_mut() {
            let entry = pending
                .iter_mut()
                .find(|it| it.key == *key)
                .ok_or_else(|| ListError::MissingFields(key.clone()))?;
            entry.fields = Some(fields);
            self.note_pending();
            return Ok(());
        }
        let item = list
            .items
            .get(key)
            .copied()
            .ok_or_else(|| ListError::MissingFields(key.clone()))?;
        let selectors = list.selectors.clone();
        for (slot, v) in fields.into_iter().enumerate() {
            self.set(item, slot as u16, v);
        }
        self.refresh_flags(instance, &selectors, item);
        Ok(())
    }

    /// Apply every staged list (the first step of a flush).
    pub(crate) fn apply_staged(&mut self) {
        for (instance, block) in std::mem::take(&mut self.staged) {
            self.apply_one(instance, block);
        }
    }

    /// Apply list `block` of `instance` if it has a staged list.
    fn apply_one(&mut self, instance: InstanceId, block: usize) {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return;
        };
        let Some(list) = inst.lists.get_mut(block) else {
            return;
        };
        let Some(items) = list.pending.take() else {
            return;
        };
        // Work on the list out of its instance (mounting borrows `self`).
        let placeholder = ListState::new(0, Arc::clone(&list.item), Vec::new());
        let mut list = std::mem::replace(list, placeholder);
        // Same order as applied: no positions to compute.
        let same_order = same_keys(&list.keys, &items);
        let new_keys: Vec<Key> = if same_order {
            std::mem::take(&mut list.keys)
        } else {
            items.iter().map(|it| it.key.clone()).collect()
        };
        let d = if same_order {
            keyed::KeyedDiff::default()
        } else {
            keyed::diff(&list.keys, &new_keys)
        };

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
                Some(&item) => {
                    fields
                        .into_iter()
                        .enumerate()
                        .for_each(|(s, v)| self.set(item, s as u16, v));
                    self.refresh_flags(instance, &list.selectors, item);
                }
                None => {
                    fresh.insert(i, fields);
                }
            }
        }
        // The selections new rows are born under.
        let selections: Vec<Value> = list
            .selectors
            .iter()
            .map(|s| self.get(instance, s.from).cloned().unwrap_or(Value::Null))
            .collect();
        for step in &d.steps {
            let (new, before) = match *step {
                Step::Insert { new, before } => {
                    let mut slots = fresh
                        .remove(&new)
                        .expect("validated: new keys carry fields");
                    slots.extend(std::iter::repeat_n(Value::Null, list.selectors.len()));
                    for (s, selection) in list.selectors.iter().zip(&selections) {
                        slots[usize::from(s.flag)] =
                            select::flag(selection, &slots[usize::from(s.field)]);
                    }
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
    }

    /// The bridge id of an item's root (node 0, checked by `Compiled::new`).
    fn root(&self, item: InstanceId) -> NodeId {
        self.instances[item].as_ref().expect("live item").id0
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
