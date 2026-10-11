//! The virtual-pointer event path: clicks, pointer/drag, hover, and
//! interaction styling for UI a feature drives with its own picking pointer
//! ([`VirtualPointers`] — a `<surface>`'s offscreen subtree). Such nodes never
//! receive a legacy `Interaction` (their offscreen camera makes
//! `ui_focus_system` skip them), so everything here rides the virtual
//! pointers' picking events instead — deliberately mirroring the main-window
//! collectors in `events.rs`/`pointer.rs`/`interaction.rs` rather than
//! sharing their code: each side's attribution quirks are documented on its
//! own systems.

use bevy::picking::events::{
    PointerClick, PointerDrag, PointerEnter, PointerLeave, PointerMove, PointerPress,
    PointerRelease,
};
use bevy::picking::pointer::PointerButton;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use super::events::{Modifiers, climb, dom_button, send_ui_event, surface_relative};
use crate::bridge::{JsBridge, PointerHandlers, ReactNode, StyleVariants};
use crate::ext::VirtualPointers;
use crate::style::Style;
use crate::ui_map::{apply_style, overlay_style};

/// Report virtual-pointer clicks to JS — the analogue of
/// [`collect_ui_events`](crate::reconcile::collect_ui_events) for UI a
/// registered virtual pointer drives (a `<surface>`'s in-world picking path
/// moves one over the offscreen subtree, so a click there arrives as a
/// `PointerClick` for that pointer; the nodes never get a legacy
/// `Interaction` press, since they don't render to a window).
/// Primary-button-only like it, and scoped to the registered
/// [`VirtualPointers`] so it never double-fires for main-window UI.
pub fn collect_virtual_clicks(
    bridge: Res<JsBridge>,
    pointers: Res<VirtualPointers>,
    mut clicks: MessageReader<PointerClick>,
    // Click ownership (see [`ClickOwners`](super::events::ClickOwners)) —
    // matching `collect_ui_events`' attribution exactly (NOT `Interaction`:
    // hover/press styling must never steal a click from an ancestor with a
    // real handler).
    targets: Query<&ReactNode, super::events::ClickOwners>,
    child_of: Query<&ChildOf>,
) {
    if pointers.is_empty() {
        return;
    }
    // A pass-through node stacked over the target makes one gesture fan out to
    // every entity in the hover map. Same no-bubbling rule as
    // `collect_ui_events`: only the topmost resolving hit (smallest
    // `HitData.depth` — hover-map message order is arbitrary) owns the click.
    let mut topmost: Option<(f32, Entity)> = None;
    for ev in clicks.read() {
        // Like DOM `click` (and `collect_ui_events`), only the primary button
        // clicks; right/middle ride the `onPointer*` events.
        if !pointers.contains(ev.pointer.id) || ev.button != PointerButton::Primary {
            continue;
        }
        // Resolve the picked leaf to the nearest interactive ancestor (the button),
        // so a click on its label text still fires the button's handler.
        if let Some(target) = climb(ev.entity, &child_of, |e| targets.contains(e))
            && !topmost.is_some_and(|(depth, _)| depth <= ev.hit.depth)
        {
            topmost = Some((ev.hit.depth, target));
        }
    }
    if let Some((_, target)) = topmost
        && let Ok(rnode) = targets.get(target)
    {
        tracing::debug!("virtual-pointer click -> reconciler node {}", rnode.0);
        send_ui_event(&bridge, rnode.0, "click", None, None, None, None);
    }
}

/// Report `onPointer*` drag events for virtual-pointer UI, mirroring
/// [`collect_pointer_events`](crate::reconcile::collect_pointer_events) for the
/// in-world picking path. Press → `pointerDown`,
/// drag → `pointerMove`, release → `pointerUp`, each gated on the node's declared
/// [`PointerHandlers`], carrying the cursor's node-relative `0..1` position
/// (the surface-space pixel as `client_x/y`) and the mouse button (a `Drag`'s
/// button is the one doing the dragging).
#[allow(clippy::too_many_arguments)]
pub fn collect_virtual_pointer_events(
    bridge: Res<JsBridge>,
    pointers: Res<VirtualPointers>,
    mut presses: MessageReader<PointerPress>,
    mut releases: MessageReader<PointerRelease>,
    mut drags: MessageReader<PointerDrag>,
    nodes: Query<(
        &ReactNode,
        &PointerHandlers,
        &ComputedNode,
        &UiGlobalTransform,
    )>,
    child_of: Query<&ChildOf>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
) {
    if pointers.is_empty() {
        return;
    }
    let mods = Modifiers::read(keys.as_deref());
    // Per-kind (owner, button) dedupe: a pass-through node stacked over the
    // target fans each gesture out to every hovered entity, and climbing can
    // resolve them to the same owner. (Moves see at most one `Drag` per button
    // per frame — a virtual pointer's driver emits at most one `Move` per frame —
    // so the set never suppresses a genuine repeat.)
    let mut seen: HashSet<(Entity, PointerButton)> = HashSet::new();
    let emit = |entity: Entity,
                want: fn(&PointerHandlers) -> bool,
                kind: &str,
                at: Vec2,
                button: PointerButton,
                seen: &mut HashSet<(Entity, PointerButton)>| {
        // Resolve the picked leaf to the nearest ancestor that declared `onPointer*`.
        if let Some(target) = climb(entity, &child_of, |e| nodes.contains(e))
            && seen.insert((target, button))
            && let Ok((rnode, handlers, node, transform)) = nodes.get(target)
            && want(handlers)
            && let Some((pos, abs)) = surface_relative(node, transform, at)
        {
            send_ui_event(
                &bridge,
                rnode.0,
                kind,
                Some(pos),
                Some(abs),
                Some(dom_button(button)),
                Some(mods),
            );
        }
    };
    for ev in presses.read() {
        if pointers.contains(ev.pointer.id) {
            emit(
                ev.entity,
                |h| h.down,
                "pointerDown",
                ev.pointer.position,
                ev.button,
                &mut seen,
            );
        }
    }
    seen.clear();
    for ev in drags.read() {
        if pointers.contains(ev.pointer.id) {
            emit(
                ev.entity,
                |h| h.moved,
                "pointerMove",
                ev.pointer.position,
                ev.button,
                &mut seen,
            );
        }
    }
    seen.clear();
    for ev in releases.read() {
        if pointers.contains(ev.pointer.id) {
            emit(
                ev.entity,
                |h| h.up,
                "pointerUp",
                ev.pointer.position,
                ev.button,
                &mut seen,
            );
        }
    }
}

/// Report `pointerEnter` / `pointerLeave` for virtual-pointer UI, mirroring
/// [`collect_virtual_pointer_events`] for the hover boundary. Such nodes get no
/// legacy `Interaction`, so this reads the virtual pointer's `PointerEnter` /
/// `PointerLeave` picking events. Those already implement DOM
/// `mouseenter`/`mouseleave` semantics — they fire for the hovered entity *and*
/// its ancestors, only on true boundary crossings — so no climb (and no dedupe)
/// is needed, and crossing between a button's label and its padding never
/// re-fires the button's boundary. Hover events carry no button.
///
/// `pointerHover` rides `PointerMove`, which bevy_picking also sends to
/// every hovered entity (ancestors included), so it needs no climb either —
/// only a per-entity dedupe, in case two virtual pointers move over one node.
#[allow(clippy::too_many_arguments)]
pub fn collect_virtual_hover_events(
    bridge: Res<JsBridge>,
    pointers: Res<VirtualPointers>,
    mut enters: MessageReader<PointerEnter>,
    mut leaves: MessageReader<PointerLeave>,
    mut moves: MessageReader<PointerMove>,
    nodes: Query<(
        &ReactNode,
        &PointerHandlers,
        &ComputedNode,
        &UiGlobalTransform,
    )>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
) {
    if pointers.is_empty() {
        return;
    }
    let mods = Modifiers::read(keys.as_deref());
    let emit = |entity: Entity, want: fn(&PointerHandlers) -> bool, kind: &str, at: Vec2| {
        if let Ok((rnode, handlers, node, transform)) = nodes.get(entity)
            && want(handlers)
            && let Some((pos, abs)) = surface_relative(node, transform, at)
        {
            send_ui_event(
                &bridge,
                rnode.0,
                kind,
                Some(pos),
                Some(abs),
                None,
                Some(mods),
            );
        }
    };
    for ev in enters.read() {
        if pointers.contains(ev.pointer.id) {
            emit(ev.entity, |h| h.enter, "pointerEnter", ev.pointer.position);
        }
    }
    for ev in leaves.read() {
        if pointers.contains(ev.pointer.id) {
            emit(ev.entity, |h| h.leave, "pointerLeave", ev.pointer.position);
        }
    }
    let mut seen: HashSet<Entity> = HashSet::new();
    for ev in moves.read() {
        if pointers.contains(ev.pointer.id) && seen.insert(ev.entity) {
            emit(ev.entity, |h| h.hover, "pointerHover", ev.pointer.position);
        }
    }
}

/// Apply hover/press [`StyleVariants`] to virtual-pointer UI (a `<surface>`'s
/// subtree) from its picking path — the analogue of
/// [`apply_interaction_styles`](crate::reconcile::apply_interaction_styles), which
/// can't help here because such nodes never receive a legacy `Interaction`
/// (their offscreen camera makes `ui_focus_system` skip them). Enter →
/// base+hover, press → base+hover+press, leave/release → base/hover — each
/// with the node's `focusStyle` overlaid last while it holds the focus, as in
/// the main-window restyle (a press or hover edge on a focused input must not
/// drop its focus overlay). The hover
/// axis rides `PointerEnter`/`PointerLeave` (boundary-only, ancestor-aware —
/// see [`collect_virtual_hover_events`]); the press axis keeps `Press`/`Release`
/// with the climb, filtered to the primary button so a right/middle press
/// doesn't trigger `pressStyle` (DOM `:active` parity with the main window's
/// `Interaction::Pressed`).
#[allow(clippy::too_many_arguments)]
pub fn apply_virtual_interaction_styles(
    mut commands: Commands,
    pointers: Res<VirtualPointers>,
    mut enters: MessageReader<PointerEnter>,
    mut leaves: MessageReader<PointerLeave>,
    mut presses: MessageReader<PointerPress>,
    mut releases: MessageReader<PointerRelease>,
    variants: Query<&StyleVariants>,
    focus: Query<&crate::bridge::FocusState>,
    child_of: Query<&ChildOf>,
    rnodes: Query<&ReactNode>,
    flags: Query<&crate::ext::ElementFlags>,
    assets: Res<AssetServer>,
    // `Option`: headless harnesses build partial apps without the bridge
    // (and some without the fonts resource).
    bridge: Option<Res<crate::bridge::JsBridge>>,
    fonts: Option<Res<crate::plugin::Fonts>>,
) {
    if pointers.is_empty() {
        return;
    }
    let default_fonts = crate::plugin::Fonts::default();
    let fonts = fonts.as_deref().unwrap_or(&default_fonts);
    let styles = bridge
        .as_ref()
        .map_or(crate::ext::core_registry(), |b| &*b.ext)
        .styles();
    let mut restyle = |entity: Entity, style: Option<Style>| {
        // Attribute re-parse warnings (e.g. a bad hoverStyle color) to the node.
        let rnode = rnodes.get(entity).ok();
        let _diag = rnode.map(|r| crate::diag::node_scope(r.0));
        let kind = match (bridge.as_ref(), rnode) {
            (Some(b), Some(r)) => b.shared_tags.kind_cow(r.0),
            _ => std::borrow::Cow::Borrowed("node"),
        };
        let info = bridge
            .as_ref()
            .map_or(crate::ext::core_registry(), |b| &*b.ext)
            .element_or_fallback(&kind);
        let attrs = match (bridge.as_ref(), rnode) {
            (Some(b), Some(r)) => b.props_cache.get(&r.0).map(|p| &p.attrs),
            _ => None,
        }
        .unwrap_or(crate::element::Attrs::empty());
        // Virtual-pointer UI (offscreen interiors) is never a promoted layer.
        let wctx = crate::style::WriterCtx {
            promoted: false,
            fresh: false,
            kind: &kind,
            flags: flags.get(entity).copied().unwrap_or(info.decl.flags),
            assets: &assets,
            fonts,
            styles,
            element: info,
            attrs,
            events: crate::element::Attrs::empty(),
            id: rnode.map_or(0, |r| r.0),
        };
        apply_style(&mut commands.entity(entity), &style, &wctx);
    };
    // Resolve a picked leaf to the nearest ancestor with hover/press variants (the
    // button), so its label text highlights the button rather than nothing.
    let target = |entity: Entity| climb(entity, &child_of, |e| variants.contains(e));
    // `focusStyle` overlays last while the node holds the focus.
    let with_focus = |entity: Entity, v: &StyleVariants, style: Option<Style>| {
        if focus.get(entity).is_ok_and(|f| f.0) {
            overlay_style(style.as_ref(), v.focus.as_ref())
        } else {
            style
        }
    };
    for ev in leaves.read() {
        if pointers.contains(ev.pointer.id)
            && let Ok(v) = variants.get(ev.entity)
        {
            restyle(
                ev.entity,
                with_focus(ev.entity, v, v.base.as_ref().cloned()),
            );
        }
    }
    for ev in enters.read() {
        if pointers.contains(ev.pointer.id)
            && let Ok(v) = variants.get(ev.entity)
        {
            let hovered = overlay_style(v.base.as_ref(), v.hover.as_ref());
            restyle(ev.entity, with_focus(ev.entity, v, hovered));
        }
    }
    for ev in releases.read() {
        if pointers.contains(ev.pointer.id)
            && ev.button == PointerButton::Primary
            && let Some(t) = target(ev.entity)
            && let Ok(v) = variants.get(t)
        {
            let hovered = overlay_style(v.base.as_ref(), v.hover.as_ref());
            restyle(t, with_focus(t, v, hovered));
        }
    }
    for ev in presses.read() {
        if pointers.contains(ev.pointer.id)
            && ev.button == PointerButton::Primary
            && let Some(t) = target(ev.entity)
            && let Ok(v) = variants.get(t)
        {
            let pressed = overlay_style(
                overlay_style(v.base.as_ref(), v.hover.as_ref()).as_ref(),
                v.press.as_ref(),
            );
            restyle(t, with_focus(t, v, pressed));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::FocusState;
    use crate::reconcile::test_util::{create, ent, op_app};
    use bevy::picking::backend::HitData;
    use bevy::picking::events::Pointer;
    use bevy::picking::pointer::{Location, PointerId, PointerMap};

    /// A press or release edge on a FOCUSED node keeps its `focusStyle`: the
    /// virtual-pointer restyle overlays it last, like the main-window one.
    /// (Clicking a `<surface>` input used to drop its focus ring on release.)
    #[test]
    fn virtual_restyle_keeps_the_focus_overlay() {
        let (mut app, tx) = op_app();
        app.init_resource::<PointerMap>()
            .add_message::<PointerEnter>()
            .add_message::<PointerLeave>()
            .add_message::<PointerPress>()
            .add_message::<PointerRelease>()
            .add_systems(Update, apply_virtual_interaction_styles);
        let pointer = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(0xF0C5));
        app.world_mut().spawn(pointer);
        app.world_mut()
            .get_resource_or_init::<VirtualPointers>()
            .register(pointer);
        tx.send(vec![create(
            1,
            "editableText",
            serde_json::json!({
                "style": { "border": 1, "borderColor": "#ff0000" },
                "focusStyle": { "borderColor": "#0000ff" },
            }),
        )])
        .unwrap();
        app.update();
        let input = ent(&app, 1);
        app.world_mut().get_mut::<FocusState>(input).unwrap().0 = true;

        let top = |app: &App| app.world().get::<BorderColor>(input).unwrap().top;
        let location = Location {
            target: bevy::camera::NormalizedRenderTarget::Image(Handle::<Image>::default().into()),
            position: Vec2::ZERO,
        };
        let hit = HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
        app.world_mut().write_message(PointerPress {
            entity: input,
            pointer: Pointer::new(pointer, location.clone()),
            button: PointerButton::Primary,
            hit: hit.clone(),
            count: 1,
        });
        app.update();
        assert_eq!(top(&app), Color::srgb(0.0, 0.0, 1.0), "pressed, focused");
        app.world_mut().write_message(PointerRelease {
            entity: input,
            pointer: Pointer::new(pointer, location),
            button: PointerButton::Primary,
            hit,
        });
        app.update();
        assert_eq!(top(&app), Color::srgb(0.0, 0.0, 1.0), "released, focused");
    }
}
