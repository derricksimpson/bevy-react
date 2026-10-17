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

use crate::command::NodeId;
use crate::compiled::Compiled;
use crate::ir::{Selector, SlotId, Window};
use crate::keyed::Key;
use crate::runtime::{InstanceId, Runtime};
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
    /// `set_window` on a list that is not windowed.
    NotWindowed,
}

/// A mounted list.
pub(crate) struct ListState {
    /// The node whose children the rows are.
    pub(crate) parent: NodeId,
    /// Rows go before this node (a bottom spacer), or last.
    pub(crate) anchor: Option<NodeId>,
    pub(crate) item: Arc<Compiled>,
    /// The mounted rows' keys, in order (all of the list unless windowed).
    pub(crate) keys: Vec<Key>,
    pub(crate) selectors: Vec<Selector>,
    pub(crate) items: HashMap<Key, InstanceId>,
    /// A list staged by [`Runtime::set_list`], applied at the next flush. Its
    /// `fields: None` entries are unchanged relative to the **applied** list.
    pub(crate) pending: Option<Vec<ListItem>>,
    /// Windowed lists only.
    pub(crate) window: Option<WindowState>,
    /// A range staged by [`Runtime::set_window`].
    pub(crate) pending_range: Option<(usize, usize)>,
    /// Windowed lists: every key in order (`keys` is the mounted slice).
    pub(crate) all: Vec<Key>,
    /// Windowed lists: the fields of the rows that are not mounted.
    pub(crate) store: HashMap<Key, Vec<Value>>,
}

/// A windowed list's slots and applied range.
pub(crate) struct WindowState {
    pub(crate) top: SlotId,
    pub(crate) bottom: SlotId,
    /// The mounted rows' index range in the full list.
    pub(crate) range: (usize, usize),
}

impl ListState {
    pub(crate) fn new(
        parent: NodeId,
        anchor: Option<NodeId>,
        item: Arc<Compiled>,
        selectors: Vec<Selector>,
        window: Option<Window>,
    ) -> Self {
        ListState {
            parent,
            anchor,
            item,
            keys: Vec::new(),
            selectors,
            items: HashMap::new(),
            pending: None,
            window: window.map(|w| WindowState {
                top: w.top,
                bottom: w.bottom,
                // Until a window is set, a windowed list mounts everything.
                range: (0, usize::MAX),
            }),
            pending_range: None,
            all: Vec::new(),
            store: HashMap::new(),
        }
    }

    /// A stand-in left in the instance while a list is applied.
    pub(crate) fn placeholder(item: &Arc<Compiled>) -> Self {
        ListState::new(0, None, Arc::clone(item), Vec::new(), None)
    }

    /// Every key of the list, mounted or not, in order.
    fn all_keys(&self) -> &[Key] {
        if self.window.is_some() {
            &self.all
        } else {
            &self.keys
        }
    }

    /// Is `key` a row of the list (mounted, or held in the store)?
    fn knows(&self, key: &Key) -> bool {
        self.items.contains_key(key) || self.store.contains_key(key)
    }

    /// How many slots an entry's fields fill (the flag slots follow).
    pub(crate) fn data_slots(&self) -> usize {
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
                let same_order = same_keys(self.all_keys(), &items);
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
                    if !same_order && it.fields.is_none() && !self.knows(&it.key) {
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

pub(crate) fn same_keys(keys: &[Key], items: &[ListItem]) -> bool {
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
        let Some(&item) = list.items.get(key) else {
            // A windowed list's unmounted row: its stored fields change.
            let stored = list
                .store
                .get_mut(key)
                .ok_or_else(|| ListError::MissingFields(key.clone()))?;
            *stored = fields;
            return Ok(());
        };
        let selectors = list.selectors.clone();
        for (slot, v) in fields.into_iter().enumerate() {
            self.set(item, slot as u16, v);
        }
        self.refresh_flags(instance, &selectors, item);
        Ok(())
    }

    /// Mount only rows `start..end` of a windowed list (clamped to its
    /// length): rows leaving the range are unmounted with their fields kept,
    /// rows entering it are mounted from them, and the spacer counts follow.
    /// Applied at the next flush, together with a staged list.
    pub fn set_window(
        &mut self,
        instance: InstanceId,
        block: usize,
        start: usize,
        end: usize,
    ) -> Result<(), ListError> {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return Err(ListError::NoSuchList);
        };
        let Some(list) = inst.lists.get_mut(block) else {
            return Err(ListError::NoSuchList);
        };
        if list.window.is_none() {
            return Err(ListError::NotWindowed);
        }
        list.pending_range = Some((start, end));
        self.staged.push((instance, block));
        self.note_pending();
        Ok(())
    }

    /// A windowed list's applied range and its full length:
    /// `(start, end, total)`.
    pub fn list_window(&self, instance: InstanceId, block: usize) -> Option<(usize, usize, usize)> {
        let list = self.instances.get(instance)?.as_ref()?.lists.get(block)?;
        let w = list.window.as_ref()?;
        let total = list.all.len();
        let (start, end) = crate::list_apply::clamp(w.range, total);
        Some((start, end, total))
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

/// The rows to mount for a scroll position: those that meet the viewport
/// plus `overscan` on each side, clamped to `total`. Rows have a fixed
/// height; feed the result to [`Runtime::set_window`].
pub fn visible_range(
    scroll_top: f64,
    viewport: f64,
    row_height: f64,
    overscan: usize,
    total: usize,
) -> (usize, usize) {
    if row_height <= 0.0 || total == 0 {
        return (0, 0);
    }
    let first = (scroll_top.max(0.0) / row_height).floor() as usize;
    let last = ((scroll_top.max(0.0) + viewport.max(0.0)) / row_height).ceil() as usize;
    (
        first.saturating_sub(overscan).min(total),
        last.saturating_add(overscan).min(total),
    )
}
