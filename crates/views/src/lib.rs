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
mod bits;
mod child;
pub mod command;
mod compiled;
mod dispatch;
mod effects;
pub mod eval;
pub mod ir;
pub mod keyed;
pub mod list;
mod list_apply;
pub mod runtime;
mod select;
pub mod value;

pub use batch::Clock;
pub use command::{Command, NodeId};
pub use compiled::Compiled;
pub use dispatch::Dispatch;
pub use effects::{EffectCall, EffectKind};
pub use ir::{
    Action, Binding, Block, ChildBlock, Effect, Expr, Handler, JsRef, Selector, SlotId, TNode,
    Target, Template, Window, e,
};
pub use keyed::Key;
pub use list::{ListError, ListItem, visible_range};
pub use runtime::{InstanceId, Runtime};
pub use value::Value;
