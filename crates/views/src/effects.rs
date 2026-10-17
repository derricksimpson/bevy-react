//! `useEffect`: JS that runs after mount and when dependencies change.
//!
//! The runtime does not run JS; it records, in React's order, what the host
//! should run, and the host collects it with [`Runtime::take_effects`] after
//! each flush. A run the host has not collected yet is cancelled if the
//! component changes again or unmounts first, as React cancels effects of a
//! render that never committed.

use crate::ir::{Effect, JsRef};
use crate::runtime::{Instance, InstanceId, Runtime};
use crate::value::Value;

/// What to do with an effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    /// Run the effect function.
    Run,
    /// Call the cleanup the previous run returned.
    Cleanup,
}

/// One call for the host, to be made in the order returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectCall {
    pub instance: InstanceId,
    /// The effect's index in its component's template.
    pub effect: u16,
    pub js: JsRef,
    pub kind: EffectKind,
}

impl Runtime {
    /// The effect calls queued since the last call, in order.
    pub fn take_effects(&mut self) -> Vec<EffectCall> {
        std::mem::take(&mut self.effects)
    }

    /// Queue every effect of a freshly mounted instance, and remember the
    /// dependency values they ran with.
    pub(crate) fn start_effects(&mut self, instance: InstanceId) {
        let Some(Some(inst)) = self.instances.get(instance) else {
            return;
        };
        let compiled = inst.compiled.clone();
        let snaps: Vec<Vec<Value>> = compiled
            .template()
            .effects
            .iter()
            .map(|fx| snapshot(fx, &inst.slots))
            .collect();
        if let Some(Some(inst)) = self.instances.get_mut(instance) {
            inst.effect_deps = snaps;
        }
        for e in 0..compiled.template().effects.len() {
            self.queue_run(instance, e);
        }
    }

    /// An effect's dependencies were written: run it again if their values
    /// differ from the last run's.
    pub(crate) fn effect_deps_changed(&mut self, instance: InstanceId, e: usize) {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return;
        };
        let compiled = inst.compiled.clone();
        let fx = &compiled.template().effects[e];
        let now = snapshot(fx, &inst.slots);
        // An effect with no dependency array runs after any write.
        if fx.deps.is_some() && inst.effect_deps[e] == now {
            return;
        }
        inst.effect_deps[e] = now;
        self.queue_run(instance, e);
    }

    /// Queue `Cleanup` (if the last run left one) then `Run`; a run still
    /// waiting for the host is replaced instead.
    fn queue_run(&mut self, instance: InstanceId, e: usize) {
        let Some(Some(inst)) = self.instances.get_mut(instance) else {
            return;
        };
        let fx = &inst.compiled.template().effects[e];
        let (js, cleanup) = (fx.run, fx.cleanup);
        let ran = std::mem::replace(&mut inst.effect_ran[e], true);
        let pending = self.effects.iter().position(|c| {
            c.instance == instance && usize::from(c.effect) == e && c.kind == EffectKind::Run
        });
        if let Some(at) = pending {
            self.effects.remove(at); // the host never saw it: nothing to clean up
        } else if ran && cleanup {
            self.effects.push(EffectCall {
                instance,
                effect: e as u16,
                js,
                kind: EffectKind::Cleanup,
            });
        }
        self.effects.push(EffectCall {
            instance,
            effect: e as u16,
            js,
            kind: EffectKind::Run,
        });
        self.note_pending();
    }

    /// Unmounting: call the cleanups of runs the host has seen, and drop the
    /// runs it has not.
    pub(crate) fn cleanup_effects(&mut self, instance: InstanceId, inst: &Instance) {
        let effects = &inst.compiled.template().effects;
        if effects.is_empty() {
            return;
        }
        for (e, fx) in effects.iter().enumerate() {
            let pending = self.effects.iter().position(|c| {
                c.instance == instance && usize::from(c.effect) == e && c.kind == EffectKind::Run
            });
            match pending {
                Some(at) => {
                    self.effects.remove(at);
                }
                None if fx.cleanup && inst.effect_ran[e] => self.effects.push(EffectCall {
                    instance,
                    effect: e as u16,
                    js: fx.run,
                    kind: EffectKind::Cleanup,
                }),
                None => {}
            }
        }
        self.note_pending();
    }
}

/// The values an effect depends on, in dependency order (empty if it has no
/// dependency array: it compares nothing).
fn snapshot(fx: &Effect, slots: &[Value]) -> Vec<Value> {
    fx.deps
        .iter()
        .flatten()
        .map(|s| slots.get(usize::from(*s)).cloned().unwrap_or(Value::Null))
        .collect()
}
