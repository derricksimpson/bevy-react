//! Props → component stamping: the helpers that mirror a node's wire props
//! into ECS components, shared by the create path (`create.rs`) and the
//! delta-update path (`update.rs`). Each "stamps (or clears)" its component so
//! a re-render converges on exactly the declared props; `fresh` (the create
//! path) skips the clears — a freshly spawned entity has nothing to remove.

use bevy::a11y::AccessibilityNode;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition};

use crate::animations::AnimatedNode;
use crate::bridge::{
    ClickOwner, FocusState, HoverState, JsBridge, PointerHandlers, ReactInteraction, Restyle,
    ScrollListener, ScrollStep, StyleVariants, WheelListener,
};
use crate::protocol::{
    NodeId,
    props::{Props, PropsDirty, UpdateEvents},
};
use crate::style::Style;
use crate::style::props::{BACKDROP_FILTER, CACHE, FILTER, MORPH_FILTER, TRANSFORM3D};
use crate::transition::ScrollTransitionState;

/// Stamp (or clear) the [`AnimatedNode`] bindings on a host element, derived
/// from the merged base style's `{ animated }` wrappers **and** — on an SVG
/// shape child — the numeric shape attrs' (`crate::style_bindings`, one
/// merged map; removed only when both sources are empty). Present → the
/// animations plugin drives the listed props each frame (no-op if animations
/// are disabled — nothing reads the component). Also warns (once per apply,
/// devtools-mirrored) about wrappers in hover/press/focus variants, which are
/// ignored by design, and about a gradient `transition` spec made inert by
/// gradient bindings on the same surface (bindings park the channel).
///
/// `animated` is the bridge's mirror of which nodes carry the component
/// ([`JsBridge::animated`]): the remove is queued only for a node that had
/// bindings — a style delta on a binding-less node (nearly every node)
/// queues nothing.
pub(crate) fn apply_animated(
    ec: &mut EntityCommands,
    animated: &mut HashSet<NodeId>,
    id: NodeId,
    props: &Props,
    fresh: bool,
) {
    crate::style_bindings::warn_variant_bindings(props);
    let bindings = crate::style_bindings::derive_props_bindings(props);
    crate::style_bindings::warn_gradient_transition_mix(props, bindings.as_ref());
    match bindings {
        Some(bindings) => {
            // The engine's publish slot for feature-owned bindings rides
            // the stamp (see `crate::ext::DrivenExtValues`).
            if bindings.has_stage(crate::animations::props::PropStage::Ext) {
                ec.insert_if_new(crate::ext::DrivenExtValues::default());
            } else if !fresh {
                ec.remove::<crate::ext::DrivenExtValues>();
            }
            ec.insert(AnimatedNode(bindings));
            animated.insert(id);
        }
        None => {
            if !fresh && !animated.is_empty() && animated.remove(&id) {
                ec.remove::<(AnimatedNode, crate::ext::DrivenExtValues)>();
            }
        }
    }
}

/// Stamp (or clear) the hover/press [`StyleVariants`] on a host element. When
/// either variant is present the element also gets a [`ReactInteraction`] so
/// `sync_interactions` tracks hover/press for it; `insert_if_new` leaves any
/// existing one untouched (a `button`'s, or a node already mid-hover) so we
/// never reset its state on a re-render. `base` is the node's effective
/// style (its element's default style overlaid by the user's).
pub(super) fn apply_style_variants(
    ec: &mut EntityCommands,
    props: &Props,
    base: &Option<Style>,
    fresh: bool,
) {
    if props.has_variant() {
        ec.insert(StyleVariants {
            base: base.clone(),
            hover: props.hover_style.clone(),
            press: props.press_style.clone(),
            focus: props.focus_style.clone(),
            focus_visible: props.focus_visible_style.clone(),
            keys: variant_keys(props),
            restyle: Restyle::Full,
        });
        // Hover/press are driven by `ReactInteraction`; focus by `FocusState`
        // (written by `sync_focus`). Add each only for the variants present.
        if props.hover_style.is_some() || props.press_style.is_some() {
            ec.insert_if_new(ReactInteraction::default());
        }
        if props.focus_style.is_some() || props.focus_visible_style.is_some() {
            ec.insert_if_new(FocusState::default());
        } else if !fresh {
            ec.remove::<FocusState>();
        }
    } else if !fresh {
        ec.remove::<(StyleVariants, FocusState)>();
    }
}

/// The delta-update form of [`apply_style_variants`]: a variant field in the
/// delta (set or unset) re-stamps/clears the component wholesale; a
/// base-style-only delta on a node that HAS variants updates
/// `StyleVariants.base` in place and records the delta's touched properties
/// ([`Restyle::note_base_delta`]) so the interaction restyle re-runs only
/// their writers — or nothing at all on an idle node; a delta on a node with no
/// variants (and none in the delta) queues nothing.
pub(super) fn apply_style_variants_delta(
    ec: &mut EntityCommands,
    props: &Props,
    base: &Option<Style>,
    dirty: &PropsDirty,
) {
    if !dirty.any_style_variant() {
        return;
    }
    if dirty.variant() {
        apply_style_variants(ec, props, base, false);
        return;
    }
    // Only base properties changed. The component exists exactly when a variant
    // is present (create and the arm above keep that invariant).
    if props.has_variant() {
        let base = base.clone();
        let mask = dirty.style;
        ec.queue(move |mut entity: EntityWorldMut| {
            if let Some(mut variants) = entity.get_mut::<StyleVariants>() {
                variants.base = base;
                variants.restyle.note_base_delta(mask);
            }
        });
    }
}

/// Stamp (or clear) the [`PointerHandlers`] marker and what goes with it.
///
/// Both `onClick` and the `onPointer*` handlers make the element interactive
/// — a [`ReactInteraction`] (hover/press state, the enter/leave boundary and
/// the [`crate::PointerCapture`] source) — **and** a click owner: the
/// [`ClickOwner`] marker
/// ([`collect_ui_events`](crate::reconcile::collect_ui_events) climbs a picked
/// leaf to the nearest owner). The two are separate on purpose: hover/press
/// styling also inserts a `ReactInteraction`, and a style-only interactive
/// element must never steal a click from an ancestor with a real handler.
/// `insert_if_new` leaves an existing `ReactInteraction` (a `button`'s, or a
/// hover/press variant's) untouched.
pub(crate) fn apply_pointer_handlers(
    ec: &mut EntityCommands,
    props: &Props,
    flags: crate::ext::ElementFlags,
    fresh: bool,
) {
    let any_pointer = props.on_pointer_down
        || props.on_pointer_move
        || props.on_pointer_up
        || props.on_pointer_enter
        || props.on_pointer_leave
        || props.on_pointer_hover;
    if any_pointer {
        let handlers = PointerHandlers {
            down: props.on_pointer_down,
            moved: props.on_pointer_move,
            up: props.on_pointer_up,
            enter: props.on_pointer_enter,
            leave: props.on_pointer_leave,
            hover: props.on_pointer_hover,
        };
        ec.insert(handlers);
    } else if !fresh {
        ec.remove::<PointerHandlers>();
    }
    // `pointerEnter`/`pointerLeave` are derived from `ReactInteraction`
    // transitions, so the node tracks which pointer is inside in `HoverState`;
    // add/remove it in step.
    if props.on_pointer_enter || props.on_pointer_leave {
        ec.insert_if_new(HoverState::default());
    } else if !fresh {
        ec.remove::<HoverState>();
    }
    if props.on_click || any_pointer {
        ec.insert_if_new((ReactInteraction::default(), ClickOwner));
    } else if !fresh {
        // Safe to remove unconditionally: `<button>`/`editableText` own clicks
        // by element type in the collectors, not through this marker.
        ec.remove::<ClickOwner>();
    }
    if flags.node_less {
        node_less_pointer_slot(ec, props.on_click || any_pointer, fresh);
    }
}

/// A node-less element (an SVG shape) has no box for the collectors to
/// measure: with a pointer handler it carries an [`EventLocalPos`] slot the
/// feature owning its coordinate space fills (the event's `x`/`y`). On full
/// handler removal its [`ReactInteraction`] goes too — unlike a layout node
/// (where it may serve hover/press styling or a `<button>`), a node-less
/// element's serves its handlers alone, and a leftover would keep it
/// counting as interactive UI under the pointer.
///
/// [`EventLocalPos`]: crate::ext::EventLocalPos
fn node_less_pointer_slot(ec: &mut EntityCommands, any: bool, fresh: bool) {
    if any {
        ec.insert_if_new(crate::ext::EventLocalPos::default());
    } else if !fresh {
        ec.remove::<(crate::ext::EventLocalPos, ReactInteraction)>();
    }
}

/// A nested `<text>` span is a Node-less text run: it has no layout box, so
/// the layer-family styles (`filter`/`backdropFilter`/`morphFilter`/
/// `transform3d`/`cache`) can never promote it — its glyphs render under the
/// enclosing `<text>` block, which is where those styles belong. A
/// structurally silent no-op; mirror it into devtools so the surprise is at
/// least visible. Call under the op's `diag::node_scope`.
pub(super) fn warn_span_ignored(props: &Props) {
    for (present, name) in [
        (
            props.all_styles().any(|s| s.get(&FILTER).is_some()),
            "filter",
        ),
        (
            props
                .all_styles()
                .any(|s| s.get(&BACKDROP_FILTER).is_some()),
            "backdropFilter",
        ),
        (
            props.all_styles().any(|s| s.get(&MORPH_FILTER).is_some()),
            "morphFilter",
        ),
        (
            props.all_styles().any(|s| s.get(&TRANSFORM3D).is_some()),
            "transform3d",
        ),
        (props.all_styles().any(|s| s.get(&CACHE).is_some()), "cache"),
    ] {
        if present {
            crate::diag::report(
                "spanLayerStyle",
                name,
                &format!(
                    "`{name}` on a nested <text> span is ignored — a span has no \
                     layout box to capture; put it on the enclosing <text> (or a \
                     wrapping <node>)"
                ),
            );
        }
    }
}

/// A style property the element ignores (only global writers its own writers
/// masked off — or it opted out of — read it: `backgroundImage` on an
/// `<image>`/`<canvas>`/`<portal>`/`<surface>`) warns `styleIgnored`, once per
/// distinct value. Call under the op's `diag::node_scope`.
pub(super) fn warn_ignored_styles(
    info: &crate::element::ElementInfo,
    styles: &crate::style::StyleRegistry,
    props: &Props,
) {
    if !info.ignored_styles.any() {
        return;
    }
    for style in props.all_styles() {
        if !style.keys().intersects(&info.ignored_styles) {
            continue;
        }
        for (property, _) in style.iter() {
            if styles
                .id_of(property)
                .is_some_and(|id| info.ignored_styles.contains(id))
            {
                let name = property.name();
                crate::diag::report(
                    "styleIgnored",
                    name,
                    &format!("`{name}` has no effect on <{}>", info.name()),
                );
            }
        }
    }
}

/// Stamp (or clear) bevy's `TabIndex` from `tabIndex` — what makes an element
/// focusable: by a press (`reconcile::focus`), by Tab (bevy's
/// `TabNavigationPlugin`, the app's opt-in) and from Rust. A text input is
/// focusable without the prop (`0`); a disabled control is never focusable.
/// `onFocus`/`onBlur` need no stamp: `sync_focus` reads them off the
/// retained props.
pub(crate) fn apply_focus(
    ec: &mut EntityCommands,
    props: &Props,
    flags: crate::ext::ElementFlags,
    fresh: bool,
) {
    let disabled = props.attrs.get(&crate::elements::DISABLED) == Some(&true);
    let implied = (flags.text == crate::ext::TextRole::Input).then_some(0);
    match props.tab_index.or(implied).filter(|_| !disabled) {
        Some(index) => {
            ec.insert(TabIndex(index));
            // bevy_winit stops updating the accessibility tree while the
            // focused entity has no node in it. A `<button>`'s or an input's
            // own node is already there.
            // ponytail: left behind when `tabIndex` is removed later.
            ec.insert_if_new(AccessibilityNode(accesskit::Node::new(
                accesskit::Role::GenericContainer,
            )));
        }
        None if !fresh => {
            ec.remove::<TabIndex>();
        }
        None => {}
    }
}

/// Toggle the [`ScrollListener`] marker so
/// [`collect_scroll_events`](crate::reconcile::collect_scroll_events) reports
/// this node's `ScrollPosition` changes only while an `onScroll` handler is
/// declared.
pub(super) fn apply_scroll_listener(ec: &mut EntityCommands, props: &Props, fresh: bool) {
    if props.on_scroll {
        ec.insert_if_new(ScrollListener);
    } else if !fresh {
        ec.remove::<ScrollListener>();
    }
}

/// Toggle the [`WheelListener`] marker so [`crate::scroll::collect_wheel_events`]
/// reports raw wheel deltas over this node only while an `onWheel` handler is
/// declared. Independent of `overflow: scroll` — any node can receive the wheel.
pub(super) fn apply_wheel_listener(ec: &mut EntityCommands, props: &Props, fresh: bool) {
    if props.on_wheel {
        ec.insert_if_new(WheelListener);
    } else if !fresh {
        ec.remove::<WheelListener>();
    }
}

/// Stamp (or clear) the per-node [`ScrollStep`] wheel step from `scrollStep`.
pub(super) fn apply_scroll_step(ec: &mut EntityCommands, props: &Props, fresh: bool) {
    match props.scroll_step {
        Some(step) => {
            ec.insert(ScrollStep(step));
        }
        None if !fresh => {
            ec.remove::<ScrollStep>();
        }
        None => {}
    }
}

/// The common prop stamps of a freshly spawned element, per the groups its
/// element declares: the style variants (over the effective style `base`),
/// the pointer handlers, the tab index, and the `{ animated }` bindings
/// (always — style and attribute bindings alike).
///
/// **Create only** — the stamp/clear helpers above with `fresh` set: the entity
/// is freshly spawned, so every "absent → remove" arm is skipped.
pub(crate) fn stamp_common(
    ec: &mut EntityCommands,
    animated: &mut HashSet<NodeId>,
    id: NodeId,
    props: &Props,
    base: &Option<Style>,
    common: crate::element::Common,
    flags: crate::ext::ElementFlags,
) {
    use crate::element::Common;
    if common.contains(Common::VARIANTS) {
        apply_style_variants(ec, props, base, true);
    }
    if common.contains(Common::POINTER) {
        apply_pointer_handlers(ec, props, flags, true);
    }
    if common.contains(Common::FOCUS) {
        apply_focus(ec, props, flags, true);
    }
    apply_animated(ec, animated, id, props, true);
}

/// The properties a node's hover/press/focus variants set.
pub(super) fn variant_keys(props: &Props) -> crate::style::StyleDirty {
    [
        &props.hover_style,
        &props.press_style,
        &props.focus_style,
        &props.focus_visible_style,
    ]
    .into_iter()
    .flatten()
    .fold(crate::style::StyleDirty::NONE, |keys, s| {
        keys.union(s.keys())
    })
}

/// Apply a controlled `scrollTop`/`scrollLeft` on **create**: insert the offset
/// (defaulting the uncontrolled axis to 0) and seed [`JsBridge::scroll_positions`]
/// so neither the programmatic write nor the node's mount-frame
/// `Changed<ScrollPosition>` echoes back as an `onScroll`. A listener with no
/// controlled offset is seeded at the default `ZERO` for the same reason.
pub(super) fn create_controlled_scroll(
    bridge: &mut JsBridge,
    ec: &mut EntityCommands,
    id: NodeId,
    props: &Props,
    events: &UpdateEvents,
) {
    if events.scroll_top.is_some() || events.scroll_left.is_some() {
        let pos = Vec2::new(
            events.scroll_left.unwrap_or(0.0),
            events.scroll_top.unwrap_or(0.0),
        );
        // Overrides the `ZERO` that `Node`'s required `ScrollPosition` defaults to.
        // No geometry exists yet to clamp against, so the raw request also gets
        // a post-layout settle (see `PendingControlledScroll`): in-range values
        // settle silently; an overshoot lands on the real max and echoes it.
        ec.insert((
            ScrollPosition(pos),
            crate::scroll::PendingControlledScroll(pos),
        ));
        bridge.scroll_positions.insert(id, pos);
    } else if props.on_scroll {
        bridge.scroll_positions.insert(id, Vec2::ZERO);
    }
}

/// Push a controlled `scrollTop`/`scrollLeft` into a live node on **update**:
/// write only the axis React controls, clamped to the scrollable range, and only
/// when it diverges from the live offset (so a re-render echoing the user's own
/// wheel scroll is a no-op and never snaps the view). Mirrors the controlled
/// `value` diff for `editableText`.
///
/// Records the **requested** (pre-clamp) value in [`JsBridge::scroll_positions`].
/// When the request was in range this equals the written offset, so the read-back
/// dedups it (no echo). When the request overshot, the clamped component value
/// diverges from the recorded request, so the read-back fires one `"scroll"` with
/// the real offset — letting a controlled `scrollTop={BIG}` settle to the true max.
///
/// The clamp here uses **pre-layout** geometry: when the same commit also
/// changed the content (a log appending rows + pinning to the bottom), the
/// stale range lands the write short — or skips it entirely (stale clamp ==
/// live offset). An out-of-range request therefore also parks a
/// [`crate::scroll::PendingControlledScroll`] marker, and
/// [`crate::scroll::settle_controlled_scroll`] redoes the clamp after layout.
///
/// With a scroll transition ([`ScrollTransitionState`] present) the clamped value
/// becomes the eased **target** instead of being written to `ScrollPosition` — the
/// `drive_scroll_transition` system moves the offset toward it. The uncontrolled
/// axis keeps the current target (not the mid-ease position) so it doesn't snap.
pub(super) fn update_controlled_scroll(
    bridge: &mut JsBridge,
    ec: &mut EntityCommands,
    scroll_query: &mut Query<(
        &mut ScrollPosition,
        &ComputedNode,
        Option<&mut ScrollTransitionState>,
    )>,
    e: Entity,
    id: NodeId,
    scroll_left: Option<f32>,
    scroll_top: Option<f32>,
) {
    if scroll_top.is_none() && scroll_left.is_none() {
        return;
    }
    if let Ok((mut pos, computed, scroll_state)) = scroll_query.get_mut(e) {
        // Base on the eased target if a transition owns the offset, else the live one.
        let mut requested = crate::scroll::scroll_base(&pos, scroll_state.as_deref());
        if let Some(x) = scroll_left {
            requested.x = x;
        }
        if let Some(y) = scroll_top {
            requested.y = y;
        }
        let clamped = requested.clamp(Vec2::ZERO, crate::scroll::scroll_range(computed));
        crate::scroll::write_scroll(&mut pos, scroll_state, clamped);
        if requested != clamped {
            // Out of the STALE range — the same commit may have grown the
            // content; redo the clamp after layout (see the doc above).
            ec.insert(crate::scroll::PendingControlledScroll(requested));
        }
        bridge.scroll_positions.insert(id, requested);
    }
}

#[cfg(test)]
mod tests;
