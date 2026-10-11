//! The two sides of the per-frame Rust↔JS boundary:
//! - [`apply_js_ops`] drains reconciler op batches and mutates the UI tree.
//! - The `collect_*` systems report interactions back to the JS thread.
//!
//! File map: `apply` (the op-apply system + lifecycle/hierarchy arms),
//! `create`/`update` (the two big op arms — one generic path each, driven by
//! the node's registered element, see [`crate::element`]), `stamps` (the
//! common prop stamps shared by both), `stats` (instrumentation + the
//! `SystemParam` bundles), `events` (click/scroll collectors + shared
//! utilities), `picking` (pointer state from bevy_picking), `pointer` (drags),
//! `hover` (enter/leave/hover), `interaction` (the hover/press/focus
//! restyle), `text_cascade` (text style inheritance). Submodules are private;
//! everything is re-exported here, so `crate::reconcile::X` is the one path.

mod apply;
mod create;
mod events;
mod hover;
mod interaction;
pub(crate) mod picking;
mod pointer;
pub(crate) mod stamps;
mod stats;
pub(crate) mod text_cascade;
mod update;

#[cfg(test)]
pub(crate) mod clip_pick_tests;
#[cfg(test)]
mod svg_tests;
#[cfg(any(test, feature = "test_util"))]
pub(crate) mod test_util;

pub use apply::apply_js_ops;
pub(crate) use events::{Modifiers, climb};
pub use events::{collect_scroll_events, collect_ui_events};
pub use hover::collect_hover_events;
pub use interaction::apply_interaction_styles;
pub use picking::{fold_span_hits, sync_interactions};
pub(crate) use pointer::ActiveDrag;
pub use pointer::collect_pointer_events;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use stats::mark_frame_start;
pub use stats::{FlushInfos, FrameStamp, OpApplyStats};
pub(crate) use update::reapply_opacity_outputs;
