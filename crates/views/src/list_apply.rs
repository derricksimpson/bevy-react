//! Applying a staged list and window to a mounted list.
//!
//! The diff always runs over the **mounted** rows: the old mounted order
//! against the part of the new list inside the window. A plain list mounts
//! all of it; a windowed one mounts a slice, so scrolling is "rows leave at
//! one end, rows enter at the other" through the same moves and inserts.

use std::collections::{HashMap, HashSet};

use crate::command::{Command, NodeId};
use crate::keyed::{self, Key, Step};
use crate::list::{ListState, same_keys};
use crate::runtime::{InstanceId, Runtime};
use crate::select;
use crate::value::Value;

impl Runtime {
    /// Apply every staged list and window (the first step of a flush).
    pub(crate) fn apply_staged(&mut self) {
        for (instance, block) in std::mem::take(&mut self.staged) {
            self.apply_one(instance, block);
        }
    }

    /// Apply list `block` of `instance` if it has a staged list or window.
    pub(crate) fn apply_one(&mut self, instance: InstanceId, block: usize) {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return;
        };
        let Some(list) = inst.lists.get_mut(block) else {
            return;
        };
        let items = list.pending.take();
        let range = list.pending_range.take();
        if items.is_none() && range.is_none() {
            return;
        }
        // Work on the list out of its instance (mounting borrows `self`).
        let placeholder = ListState::placeholder(&list.item);
        let mut list = std::mem::replace(list, placeholder);
        let windowed = list.window.is_some();
        let all_changed = items.is_some();
        let data = list.data_slots();

        // The full order afterwards, and the part of it that gets mounted.
        let mut new_all: Vec<Key> = Vec::new();
        let (mut start, mut end) = (0, 0);
        let (visible, same_order) = if windowed {
            new_all = match &items {
                Some(it) => it.iter().map(|i| i.key.clone()).collect(),
                None => std::mem::take(&mut list.all),
            };
            let w = list.window.as_mut().expect("windowed");
            // The requested range persists; only the slice is clamped, so a
            // list that is cleared and refilled gets its window back.
            w.range = range.unwrap_or(w.range);
            (start, end) = clamp(w.range, new_all.len());
            let visible = new_all[start..end].to_vec();
            let same = visible == list.keys;
            (visible, same)
        } else {
            let it = items
                .as_deref()
                .expect("a plain list only has a staged list");
            if same_keys(&list.keys, it) {
                // Same order as applied: no positions to compute.
                (std::mem::take(&mut list.keys), true)
            } else {
                (it.iter().map(|i| i.key.clone()).collect(), false)
            }
        };
        let d = if same_order {
            keyed::KeyedDiff::default()
        } else {
            keyed::diff(&list.keys, &visible)
        };
        // Which keys the list still has (only a changed windowed list can
        // lose keys that are not mounted).
        let live: Option<HashSet<&Key>> =
            (windowed && all_changed).then(|| new_all.iter().collect());

        for &o in &d.removed {
            let key = &list.keys[o];
            let item = list.items.remove(key).expect("old key is mounted");
            if windowed && live.as_ref().is_none_or(|l| l.contains(key)) {
                // Left the window, not the list: keep its fields.
                let fields =
                    self.instances[item].as_ref().expect("live row").slots[..data].to_vec();
                list.store.insert(key.clone(), fields);
            }
            self.out.push(Command::Remove {
                parent: list.parent,
                child: self.root(item),
            });
            self.forget(item);
        }

        let mut fresh: HashMap<Key, Vec<Value>> = HashMap::new();
        if let Some(items) = items {
            let in_view: HashSet<&Key> = if windowed {
                visible.iter().collect()
            } else {
                HashSet::new()
            };
            for it in items {
                let Some(fields) = it.fields else { continue };
                match list.items.get(&it.key) {
                    Some(&item) => {
                        fields
                            .into_iter()
                            .enumerate()
                            .for_each(|(s, v)| self.set(item, s as u16, v));
                        self.refresh_flags(instance, &list.selectors, item);
                    }
                    None if windowed && !in_view.contains(&it.key) => {
                        list.store.insert(it.key, fields);
                    }
                    None => {
                        fresh.insert(it.key, fields);
                    }
                }
            }
        }
        if let Some(live) = &live {
            list.store.retain(|k, _| live.contains(k));
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
                    let key = &visible[new];
                    // Fresh fields win over whatever was kept for the key.
                    let kept = list.store.remove(key);
                    let mut slots = fresh
                        .remove(key)
                        .or(kept)
                        .expect("validated: a mounted-to-be row has fields");
                    slots.extend(std::iter::repeat_n(Value::Null, list.selectors.len()));
                    for (s, selection) in list.selectors.iter().zip(&selections) {
                        slots[usize::from(s.flag)] =
                            select::flag(selection, &slots[usize::from(s.field)]);
                    }
                    let item = self.mount_detached(&list.item, list.parent, Some(instance), slots);
                    list.items.insert(key.clone(), item);
                    (new, before)
                }
                Step::Move { new, before, .. } => (new, before),
            };
            let child = self.root(list.items[&visible[new]]);
            self.out.push(match (before, list.anchor) {
                (Some(b), _) => Command::Insert {
                    parent: list.parent,
                    child,
                    before: self.root(list.items[&visible[b]]),
                },
                (None, Some(before)) => Command::Insert {
                    parent: list.parent,
                    child,
                    before,
                },
                (None, None) => Command::Append {
                    parent: list.parent,
                    child,
                },
            });
        }

        list.keys = visible;
        let spacers = list.window.as_ref().map(|w| (w.top, w.bottom));
        if windowed {
            list.all = new_all;
        }
        let total = list.all.len();
        if let Some(Some(inst)) = self.instances.get_mut(instance) {
            inst.lists[block] = list;
        }
        if let Some((top, bottom)) = spacers {
            self.set(instance, top, Value::Num(start as f64));
            self.set(instance, bottom, Value::Num((total - end) as f64));
        }
    }

    /// The bridge id of an item's root (node 0, checked by `Compiled::new`).
    pub(crate) fn root(&self, item: InstanceId) -> NodeId {
        self.instances[item].as_ref().expect("live item").id0
    }
}

/// A requested window as the slice of a list of `len` rows it selects.
pub(crate) fn clamp((start, end): (usize, usize), len: usize) -> (usize, usize) {
    let start = start.min(len);
    (start, end.min(len).max(start))
}
