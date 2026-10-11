//! Event reporting back to JS — clicks, scroll offsets — plus the small
//! shared utilities every event collector leans on ([`send_ui_event`],
//! [`climb`]).

use bevy::picking::events::PointerClick;
use bevy::picking::pointer::{PointerButton, PointerId};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::ScrollPosition;

use crate::bridge::{ClickOwner, JsBridge, ReactNode, ScrollListener};
use crate::protocol::{NodeId, outbound::Outbound, outbound::UiEvent};

/// Query filter matching every click-owning element: one with a declared
/// click/pointer handler ([`ClickOwner`], stamped by `apply_pointer_handlers`),
/// a native `<button>`, or an `editableText` input. Deliberately NOT
/// `ReactInteraction` — hover/press styling inserts one too, and a style-only
/// interactive leaf (a hover-styled `<text>` label) must never steal the
/// click from the ancestor that declared the handler.
pub(super) type ClickOwners = Or<(
    With<ClickOwner>,
    With<bevy::ui_widgets::Button>,
    With<EditableText>,
)>;

/// Report clicks on reconciler-owned nodes to the JS thread, for every
/// pointer (a `<surface>`'s included). Rides bevy_picking's
/// `PointerClick`, which fires on *release over the same node the press landed
/// on* — DOM click semantics, so press → drag off → release never clicks. Like
/// DOM `click`, only the primary (left) button clicks; right/middle interactions
/// are the `onPointer*` events' job (which carry the button).
/// A touch whose scroll gesture moved past the tap slop is excluded (web
/// semantics: scrolling cancels the tap — `pointerUp` still fires).
///
/// **A drag owns its click.** bevy_picking clicks everything that was pressed
/// and is hovered at the release, and a pass-through stack presses every
/// ancestor — so a slider released over the card around it would click the
/// card. While an element is dragged ([`ActiveDrag`]), its device's click
/// cannot come from what lay *beneath* it at the press: only the element, a
/// descendant (a `<button>` inside a press surface), or what the press hit
/// above it (a clickable badge laid over it) still clicks. Runs after
/// `collect_pointer_events`, so the gesture's `pointerUp` goes out before
/// its `click`, as in the DOM.
///
/// A disabled control (`InteractionDisabled`: a `<button disabled>`) still
/// owns the click that lands on it, and swallows it — nothing is sent, not
/// to an ancestor's `onClick` either (a DOM disabled button).
#[allow(clippy::too_many_arguments)]
pub fn collect_ui_events(
    bridge: Res<JsBridge>,
    // `Option` so headless tests needn't init them (production:
    // `ReactUiPlugin`).
    active_drag: Option<Res<super::ActiveDrag>>,
    transform3d: Option<Res<crate::layer::pick3d::Transform3dPointer>>,
    // `Option` so the many headless tests that never touch scrolling don't
    // have to init it; production always has it (`ReactUiPlugin`).
    touch_scroll: Option<Res<crate::touch_scroll::TouchScrollState>>,
    mut clicks: MessageReader<PointerClick>,
    // Click ownership (see [`ClickOwners`]).
    targets: Query<&ReactNode, ClickOwners>,
    disabled: Query<(), With<bevy::ui::InteractionDisabled>>,
    child_of: Query<&ChildOf>,
) {
    // One gesture fans out to every entity in the pointer's hover map (a
    // pass-through node stacks the whole ancestor chain under the cursor).
    // Clicks do NOT bubble: per pointer, only the topmost resolving hit — the
    // one with the smallest `HitData.depth` (bevy_ui's backend assigns 0.0 to
    // the topmost node, +ε per node beneath) — owns the click. Depth, not
    // arrival order, decides: the hover map is a HashMap, so message order
    // carries no meaning.
    let mut topmost: HashMap<PointerId, (f32, Entity)> = HashMap::new();
    for ev in clicks.read() {
        if ev.button != PointerButton::Primary {
            continue;
        }
        // A scrolled touch consumed its tap: bevy_picking emits `Click` on
        // entity identity alone (no movement threshold), so a 200px scroll
        // gesture would otherwise "click" the row it started on. Suppression
        // is readable here in either Update order relative to
        // `apply_touch_scroll` — the slop latches on move frames and the
        // claim survives the release frame (see `TouchScrollState`). A touch
        // bound to a handler drag (`ActiveDrag`) past slop still clicks —
        // pre-existing behavior, out of scope here.
        if let PointerId::Touch(id) = ev.pointer.id
            && touch_scroll.as_ref().is_some_and(|s| s.is_suppressed(id))
        {
            continue;
        }
        // Resolve the picked leaf (often a text span) to the nearest
        // click-owning ancestor, so a click on a button's label still fires
        // the button.
        if let Some(target) = climb(ev.entity, &child_of, |e| targets.contains(e)) {
            let of = super::picking::device(ev.pointer.id, transform3d.as_deref());
            if let Some(drag) = active_drag
                .as_ref()
                .and_then(|active| active.of_device(of, transform3d.as_deref()))
                && target != drag.entity
                && !drag.above.contains(&ev.entity)
                && !child_of.iter_ancestors(target).any(|a| a == drag.entity)
            {
                continue;
            }
            let candidate = (ev.hit.depth, target);
            topmost
                .entry(ev.pointer.id)
                .and_modify(|best| {
                    if candidate.0 < best.0 {
                        *best = candidate;
                    }
                })
                .or_insert(candidate);
        }
    }
    for (_, (_, target)) in topmost {
        if disabled.contains(target) {
            continue;
        }
        if let Ok(rnode) = targets.get(target) {
            tracing::debug!("click -> reconciler node {}", rnode.0);
            send_ui_event(&bridge, rnode.0, "click", None, None, None, None);
        }
    }
}

/// Report `ScrollPosition` changes back to JS as `"scroll"` events. Scoped to
/// nodes carrying a [`ScrollListener`] (i.e. those with an `onScroll` handler) so
/// the `Changed<ScrollPosition>` query stays cheap — `ScrollPosition` is a
/// required component of every `Node`, so an unscoped query would fire for every
/// node on its mount frame. A controlled write-back is deduped against
/// [`JsBridge::scroll_positions`], breaking the controlled-component echo loop.
#[allow(clippy::type_complexity)]
pub fn collect_scroll_events(
    mut bridge: ResMut<JsBridge>,
    query: Query<(&ScrollPosition, &ReactNode), (With<ScrollListener>, Changed<ScrollPosition>)>,
) {
    for (scroll, rnode) in &query {
        let id = rnode.0;
        if bridge.scroll_positions.get(&id) == Some(&scroll.0) {
            // Our own controlled write (or an unchanged value) — don't echo it.
            continue;
        }
        bridge.scroll_positions.insert(id, scroll.0);
        tracing::debug!("scroll -> reconciler node {id}");
        let _ = bridge.outbound_tx.send(Outbound::UiEvent {
            event: UiEvent {
                id,
                kind: "scroll".to_string(),
                scroll_top: Some(scroll.0.y),
                scroll_left: Some(scroll.0.x),
                ..default()
            },
        });
    }
}

/// The modifier keys held while a pointer or wheel event fired — the DOM
/// `MouseEvent` `shiftKey`/`ctrlKey`/`altKey`/`metaKey` flags. Read from this
/// frame's [`ButtonInput<KeyCode>`]; a missing resource (a headless harness
/// without input) reads as nothing held.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub(crate) fn read(keys: Option<&ButtonInput<KeyCode>>) -> Self {
        let Some(keys) = keys else {
            return Self::default();
        };
        Self {
            shift: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            ctrl: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
            alt: keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
            meta: keys.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight]),
        }
    }

    /// Stamp the four flags onto an outgoing event.
    pub(crate) fn stamp(self, event: &mut UiEvent) {
        event.shift_key = Some(self.shift);
        event.ctrl_key = Some(self.ctrl);
        event.alt_key = Some(self.alt);
        event.meta_key = Some(self.meta);
    }
}

/// Send one [`Outbound::UiEvent`] to the JS thread for a reconciler node.
/// Pointer events pass the held [`Modifiers`]; `"click"` passes `None`.
pub(super) fn send_ui_event(
    bridge: &JsBridge,
    id: NodeId,
    kind: &str,
    pos: Option<Vec2>,
    abs: Option<Vec2>,
    button: Option<u8>,
    mods: Option<Modifiers>,
) {
    let mut event = UiEvent {
        id,
        kind: kind.to_string(),
        x: pos.map(|p| p.x),
        y: pos.map(|p| p.y),
        client_x: abs.map(|a| a.x),
        client_y: abs.map(|a| a.y),
        button,
        ..default()
    };
    if let Some(mods) = mods {
        mods.stamp(&mut event);
    }
    let _ = bridge.outbound_tx.send(Outbound::UiEvent { event });
}

/// DOM `MouseEvent.button` number for a picking button (`0`/`1`/`2` =
/// left/middle/right — bevy_picking never forwards Back/Forward/Other).
pub(super) fn dom_button(button: PointerButton) -> u8 {
    match button {
        PointerButton::Primary => 0,
        PointerButton::Middle => 1,
        PointerButton::Secondary => 2,
    }
}

/// Walk up the `ChildOf` chain from `entity` (inclusive) to the nearest entity that
/// satisfies `is_target`. Surface picking hits the topmost leaf node (e.g. a `<text>`
/// inside a `<button>`); this resolves it to the node that actually owns the
/// interaction — the nearest click owner (`ClickOwner`/`Button`/`EditableText`).
/// Stops at the (detached) surface root when nothing matches.
pub(crate) fn climb(
    mut entity: Entity,
    child_of: &Query<&ChildOf>,
    is_target: impl Fn(Entity) -> bool,
) -> Option<Entity> {
    loop {
        if is_target(entity) {
            return Some(entity);
        }
        entity = child_of.get(entity).ok()?.parent();
    }
}

#[cfg(test)]
mod tests;
