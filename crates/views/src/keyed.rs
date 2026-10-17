//! Keyed list reconciliation: the old key order → the new one, with the
//! fewest moves.
//!
//! Items whose old positions form the longest increasing subsequence (LIS) of
//! the new order stay where they are; every other surviving item moves, and
//! keys not seen before are inserted. So a swap of two rows is two moves, a
//! removal is no move, and a reversal of n rows is n − 1 moves.

use crate::fast::{FastMap, map_with_capacity, set_with_capacity};
use std::sync::Arc;

/// A list item's identity (the JSX `key`: a number or a string).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Int(i64),
    Str(Arc<str>),
}

impl From<i64> for Key {
    fn from(k: i64) -> Self {
        Key::Int(k)
    }
}

impl From<&str> for Key {
    fn from(k: &str) -> Self {
        Key::Str(Arc::from(k))
    }
}

/// One positioning step. `before` is the **new** index of the item to land
/// in front of; `None` appends. Steps run in order; each anchor is already in
/// its final place when its step runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// `new[new]` was not in the old list.
    Insert { new: usize, before: Option<usize> },
    /// `new[new]` was at `old[old]` and is out of order.
    Move {
        new: usize,
        old: usize,
        before: Option<usize>,
    },
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct KeyedDiff {
    /// Old indices of the keys that left; apply these first.
    pub removed: Vec<usize>,
    /// Positioning steps, after the removals.
    pub steps: Vec<Step>,
}

/// Diff two key orders. Both must be free of duplicates (the runtime checks).
pub fn diff(old: &[Key], new: &[Key]) -> KeyedDiff {
    // Nothing to compare: everything leaves, or everything arrives in order.
    if new.is_empty() {
        return KeyedDiff {
            removed: (0..old.len()).collect(),
            steps: Vec::new(),
        };
    }
    if old.is_empty() {
        return KeyedDiff {
            removed: Vec::new(),
            steps: (0..new.len())
                .map(|new| Step::Insert { new, before: None })
                .collect(),
        };
    }
    let mut old_index: FastMap<&Key, usize> = map_with_capacity(old.len());
    old_index.extend(old.iter().enumerate().map(|(i, k)| (k, i)));
    let mut kept = set_with_capacity(new.len());
    kept.extend(new.iter());
    let removed = (0..old.len())
        .filter(|&i| !kept.contains(&old[i]))
        .collect();

    // Where each new item was before (`None` = it is new).
    let source: Vec<Option<usize>> = new.iter().map(|k| old_index.get(k).copied()).collect();
    let stable = lis_mask(&source);

    // Past the last stable item nothing needs an anchor: those items are
    // appended in order. Before it, walk backwards so each item lands in
    // front of its (already placed) successor.
    let tail = stable.iter().rposition(|&s| s).map_or(0, |last| last + 1);
    let step = |i: usize, before: Option<usize>| match source[i] {
        None => Step::Insert { new: i, before },
        Some(old) => Step::Move {
            new: i,
            old,
            before,
        },
    };
    let mut steps: Vec<Step> = (0..tail)
        .rev()
        .filter(|&i| !stable[i])
        .map(|i| step(i, Some(i + 1)))
        .collect();
    steps.extend((tail..new.len()).map(|i| step(i, None)));
    KeyedDiff { removed, steps }
}

/// `mask[i]` = `source[i]` is part of a longest increasing subsequence of the
/// `Some` entries (O(n log n), patience sorting with back links).
fn lis_mask(source: &[Option<usize>]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new(); // positions in `source`
    let mut prev: Vec<Option<usize>> = vec![None; source.len()];
    for (i, s) in source.iter().enumerate() {
        let Some(v) = *s else { continue };
        let at = tails.partition_point(|&t| source[t].is_some_and(|tv| tv < v));
        if at > 0 {
            prev[i] = Some(tails[at - 1]);
        }
        if at == tails.len() {
            tails.push(i);
        } else {
            tails[at] = i;
        }
    }
    let mut mask = vec![false; source.len()];
    let mut cur = tails.last().copied();
    while let Some(i) = cur {
        mask[i] = true;
        cur = prev[i];
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(ks: &[i64]) -> Vec<Key> {
        ks.iter().map(|&k| Key::Int(k)).collect()
    }

    fn moves(d: &KeyedDiff) -> usize {
        d.steps
            .iter()
            .filter(|s| matches!(s, Step::Move { .. }))
            .count()
    }

    #[test]
    fn swap_far_apart_is_two_moves() {
        let d = diff(&keys(&[0, 1, 2, 3, 4, 5]), &keys(&[0, 4, 2, 3, 1, 5]));
        assert_eq!(d.removed, Vec::<usize>::new());
        assert_eq!(
            d.steps,
            vec![
                Step::Move {
                    new: 4,
                    old: 1,
                    before: Some(5)
                },
                Step::Move {
                    new: 1,
                    old: 4,
                    before: Some(2)
                },
            ]
        );
    }

    #[test]
    fn removals_never_move() {
        let d = diff(&keys(&[0, 1, 2, 3, 4]), &keys(&[0, 2, 4]));
        assert_eq!(d.removed, vec![1, 3]);
        assert_eq!(d.steps, vec![]);
    }

    #[test]
    fn appends_come_out_in_order() {
        let d = diff(&keys(&[0, 1]), &keys(&[0, 1, 2, 3]));
        assert_eq!(
            d.steps,
            vec![
                Step::Insert {
                    new: 2,
                    before: None
                },
                Step::Insert {
                    new: 3,
                    before: None
                }
            ]
        );
    }

    #[test]
    fn reversal_keeps_one() {
        let d = diff(&keys(&[0, 1, 2, 3, 4]), &keys(&[4, 3, 2, 1, 0]));
        assert_eq!(moves(&d), 4);
    }

    #[test]
    fn an_empty_side_needs_no_comparison() {
        let all = keys(&[4, 5, 6]);
        let d = diff(&[], &all);
        assert_eq!(d.removed, Vec::<usize>::new());
        assert_eq!(
            d.steps,
            (0..3)
                .map(|new| Step::Insert { new, before: None })
                .collect::<Vec<_>>()
        );
        let d = diff(&all, &[]);
        assert_eq!(d.removed, vec![0, 1, 2]);
        assert_eq!(d.steps, vec![]);
        assert_eq!(diff(&[], &[]), KeyedDiff::default());
    }

    #[test]
    fn lis_skips_new_items() {
        assert_eq!(
            lis_mask(&[Some(2), None, Some(0), Some(1), None, Some(3)]),
            vec![false, false, true, true, false, true]
        );
    }
}
