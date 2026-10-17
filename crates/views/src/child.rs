//! Nested components: [`Runtime::sync_child`] brings one `ChildBlock` in line
//! with its condition and props (see [`crate::ir::ChildBlock`]).

use crate::command::Command;
use crate::eval::eval;
use crate::runtime::{InstanceId, Runtime};
use crate::value::Value;

impl Runtime {
    /// Mount, unmount or re-prop child block `c` of `instance` so it matches
    /// the instance's current slots.
    pub(crate) fn sync_child(&mut self, instance: InstanceId, c: usize) {
        let Some(Some(inst)) = self.instances.get(instance) else {
            return;
        };
        let compiled = inst.compiled.clone();
        let block = &compiled.template().children[c];
        let want = block
            .when
            .as_ref()
            .is_none_or(|w| eval(w, &inst.slots).truthy());
        let parent = inst.node(usize::from(block.parent));
        let before = block.before.map(|b| inst.node(usize::from(b)));
        let have = inst.children[c];
        let props: Vec<(u16, Value)> = if want {
            block
                .props
                .iter()
                .map(|(slot, e)| (*slot, eval(e, &inst.slots)))
                .collect()
        } else {
            Vec::new()
        };
        match (want, have) {
            (true, None) => {
                let mut slots = block.component.template().slots.clone();
                for (slot, v) in props {
                    slots[usize::from(slot)] = v;
                }
                let child = self.mount_detached(&block.component, parent, slots);
                let root = self.instances[child].as_ref().expect("just mounted").id0;
                self.out.push(match before {
                    Some(before) => Command::Insert {
                        parent,
                        child: root,
                        before,
                    },
                    None => Command::Append {
                        parent,
                        child: root,
                    },
                });
                self.instances[instance]
                    .as_mut()
                    .expect("live parent")
                    .children[c] = Some(child);
            }
            (false, Some(child)) => {
                let root = self.instances[child].as_ref().expect("live child").id0;
                self.out.push(Command::Remove {
                    parent,
                    child: root,
                });
                self.forget(child);
                self.instances[instance]
                    .as_mut()
                    .expect("live parent")
                    .children[c] = None;
            }
            (true, Some(child)) => {
                for (slot, v) in props {
                    self.set(child, slot, v);
                }
            }
            (false, None) => {}
        }
    }
}
