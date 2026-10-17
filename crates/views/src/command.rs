//! What the runtime emits: one [`Command`] per wire op it would send.
//!
//! Each variant maps one-to-one onto `bevy_react_core::protocol::Op`
//! (`Create`, `Append`, `Remove`, `UpdateText`; `SetProp`/`SetStyle` are one
//! field of an `Op::Update` delta), so a later adapter can feed the existing
//! `apply_js_ops` path unchanged.

use std::sync::Arc;

use crate::value::Value;

/// A bridge node id, the same id space as React's `allocId`.
pub type NodeId = u32;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Create {
        id: NodeId,
        kind: &'static str,
        /// Wire props; a handler crosses as `onClick: true`.
        props: Vec<(String, Value)>,
        style: Vec<(&'static str, Value)>,
        /// Inline text (`Op::Create`'s `text`).
        text: Option<Arc<str>>,
    },
    Append {
        parent: NodeId,
        child: NodeId,
    },
    Remove {
        parent: NodeId,
        child: NodeId,
    },
    SetProp {
        id: NodeId,
        key: &'static str,
        value: Value,
    },
    SetStyle {
        id: NodeId,
        key: &'static str,
        value: Value,
    },
    UpdateText {
        id: NodeId,
        text: Arc<str>,
    },
}
