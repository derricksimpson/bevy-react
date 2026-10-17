//! Selectors: `selected === row.id` for every row of a list at the cost of
//! the two rows that change (see [`Selector`]).

use crate::ir::Selector;
use crate::keyed::Key;
use crate::runtime::{InstanceId, Runtime};
use crate::value::Value;

/// The list key a selection value names: an integral number or a string.
/// Anything else (`null`, booleans, fractions) selects no row.
pub(crate) fn key_of(v: &Value) -> Option<Key> {
    match v {
        Value::Num(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 => {
            Some(Key::Int(*n as i64))
        }
        Value::Str(s) => Some(Key::Str(s.clone())),
        _ => None,
    }
}

/// Is a row whose key field is `field` selected by `selection`?
pub(crate) fn flag(selection: &Value, field: &Value) -> Value {
    Value::Bool(key_of(selection).is_some() && selection == field)
}

impl Runtime {
    /// A parent slot that lists select on changed from `old` to `new`: flip
    /// the old row off and the new row on (rows not mounted yet get their flag
    /// when they are created).
    pub(crate) fn retarget(&mut self, instance: InstanceId, slot: u16, old: Value, new: Value) {
        let Some(Some(inst)) = self.instances.get(instance) else {
            return;
        };
        let mut flips: Vec<(InstanceId, u16, bool)> = Vec::new();
        for &(block, sel) in &inst.compiled.selector_index[usize::from(slot)] {
            let list = &inst.lists[block];
            let flag_slot = list.selectors[sel].flag;
            for (value, on) in [(&old, false), (&new, true)] {
                if let Some(&item) = key_of(value).and_then(|k| list.items.get(&k)) {
                    flips.push((item, flag_slot, on));
                }
            }
        }
        for (item, flag_slot, on) in flips {
            self.set(item, flag_slot, Value::Bool(on));
        }
    }

    /// Recompute `item`'s flags against `parent`'s current selections (after
    /// its key fields were rewritten).
    pub(crate) fn refresh_flags(
        &mut self,
        parent: InstanceId,
        selectors: &[Selector],
        item: InstanceId,
    ) {
        for s in selectors {
            let selection = self.get(parent, s.from).cloned().unwrap_or(Value::Null);
            let field = self.get(item, s.field).cloned().unwrap_or(Value::Null);
            self.set(item, s.flag, flag(&selection, &field));
        }
    }
}
