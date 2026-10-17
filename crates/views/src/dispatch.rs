//! Events: [`Runtime::dispatch`] runs a node's handler.
//!
//! A Rust handler (`Handler::Do`) applies its actions on the spot, so a click
//! that only flips state never leaves Rust: the writes mark bindings, and the
//! next flush sends the change. A handler that ends in JS (or is JS) is
//! returned for the host to run, after its Rust part has been applied.

use crate::command::NodeId;
use crate::eval::eval;
use crate::ir::{Action, Handler, JsRef};
use crate::runtime::{InstanceId, Runtime};

/// What dispatching an event did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dispatch {
    /// No such node or handler.
    None,
    /// Handled in Rust; nothing for the host to run.
    Done,
    /// The host should run JS function `js`, with `instance`'s state at hand.
    Js { js: JsRef, instance: InstanceId },
}

impl Runtime {
    /// Run the `event` handler of bridge node `node`.
    pub fn dispatch(&mut self, node: NodeId, event: &str) -> Dispatch {
        let Some((instance, inst)) = self.instance_of(node) else {
            return Dispatch::None;
        };
        let compiled = inst.compiled.clone();
        let handlers = &compiled.template().nodes[(node - inst.id0) as usize].handlers;
        let Some((_, handler)) = handlers.iter().find(|(ev, _)| *ev == event) else {
            return Dispatch::None;
        };
        match handler {
            Handler::Js(js) => Dispatch::Js { js: *js, instance },
            Handler::Do { actions, then } => {
                // Read everything first, then write: one handler, one state.
                let writes: Vec<(InstanceId, u16, _)> = actions
                    .iter()
                    .filter_map(|Action::Set { up, slot, expr }| {
                        let mut target = instance;
                        for _ in 0..*up {
                            target = self.instances[target].as_ref()?.owner?;
                        }
                        let value = eval(expr, &self.instances[instance].as_ref()?.slots);
                        Some((target, *slot, value))
                    })
                    .collect();
                for (target, slot, value) in writes {
                    self.set(target, slot, value);
                }
                match then {
                    Some(js) => Dispatch::Js { js: *js, instance },
                    None => Dispatch::Done,
                }
            }
        }
    }
}
