//! Experimental compiled-views runtime (`.scratch/compiled-views/spec.md`).
//!
//! A compiled component is a [`Template`](ir::Template): a static tree plus
//! dynamic holes, each a pure [`Expr`](ir::Expr) over the instance's state
//! slots. The [`Runtime`](runtime::Runtime) mounts templates, takes state
//! writes, and emits [`Command`](command::Command)s that mirror the bridge's
//! wire ops one-to-one — the reconciliation React does in JS, done in Rust
//! without re-rendering.
//!
//! No Bevy dependency yet: the first goal is the runtime's semantics, pinned
//! by exact command streams in tests.

pub mod batch;
pub mod command;
pub mod eval;
pub mod ir;
pub mod keyed;
pub mod list;
pub mod runtime;
pub mod value;

pub use batch::Clock;
pub use command::{Command, NodeId};
pub use ir::{Binding, Block, Expr, JsRef, SlotId, TNode, Target, Template, e};
pub use keyed::Key;
pub use list::{ListError, ListItem};
pub use runtime::{Compiled, InstanceId, Runtime};
pub use value::Value;
