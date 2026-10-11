//! `positionType: "fixed"` on bevy's `FixedNode`.
//!
//! Bevy lays a `FixedNode` out as its own layout root: against the viewport
//! of its UI camera, taking no space in its parent, with none of its
//! ancestors' clips, scroll offsets or transforms (`ui_layout_system`,
//! `update_clipping_system`). It stays a child for everything else — the
//! camera it targets, visibility, stack order, events, despawn.
//!
//! The style asks for it with [`PositionFixed`] (and `PositionType::Absolute`
//! in the `Node` — in bevy's implicit viewport grid a relative item's insets
//! are nudges; only an absolute one is pinned to an edge by them), and
//! [`sync_fixed_nodes`] is the one writer of the component. Every engine
//! that assumes a child lives in its parent's frame treats it as a boundary:
//! layer membership (`crate::layer`), the transformed-layer picking remap,
//! the layout transition walk (`crate::transition::layout`).

use bevy::prelude::*;

use crate::bridge::ReactNode;

/// The node's style asks for `positionType: "fixed"`
/// (`style::writers::POSITION_FIXED_WRITER`).
#[derive(Component, Debug, Clone, Copy)]
pub struct PositionFixed;

/// bevy's `FixedNode` on a React node: present iff the node asks for it
/// ([`PositionFixed`]) and no ancestor is `display: none`.
///
/// Bevy lays a fixed node out as its own root and discards its inherited
/// clip, so a hidden ancestor would not hide it. Without the component it is
/// a plain absolute child, and gets the zero size and the full clip every
/// hidden descendant gets.
///
/// Runs in `PostUpdate` before layout. Nothing runs while no node is fixed;
/// with one mounted, the change filters cost two tick compares per UI node
/// per frame, and the ancestors are walked only on a frame where a `Node` or
/// a parent link did change.
#[allow(clippy::type_complexity)]
pub fn sync_fixed_nodes(
    mut commands: Commands,
    nodes: Query<
        (Entity, Has<PositionFixed>, Has<FixedNode>),
        (With<ReactNode>, Or<(With<PositionFixed>, With<FixedNode>)>),
    >,
    changed: Query<(), Or<(Changed<Node>, Changed<ChildOf>, Added<PositionFixed>)>>,
    mut dropped: RemovedComponents<PositionFixed>,
    parents: Query<&ChildOf>,
    displays: Query<&Node>,
) {
    if nodes.is_empty() {
        return;
    }
    let dropped = dropped.read().count() > 0;
    if !dropped && changed.is_empty() {
        return;
    }
    for (entity, wanted, fixed) in &nodes {
        let want = wanted
            && !parents
                .iter_ancestors(entity)
                .any(|a| displays.get(a).is_ok_and(|n| n.display == Display::None));
        if want && !fixed {
            commands.entity(entity).insert(FixedNode);
        } else if !want && fixed {
            commands.entity(entity).remove::<FixedNode>();
        }
    }
}

#[cfg(test)]
mod tests;
