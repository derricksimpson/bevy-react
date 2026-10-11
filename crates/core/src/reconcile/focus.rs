//! Keyboard focus: which element a pointer press focuses
//! ([`resolve_pointer_focus`]), and what React sees of bevy's `InputFocus`
//! ([`sync_focus`]: the `focusStyle` overlay, `onFocus`/`onBlur`). An element
//! is focusable when it carries bevy's `TabIndex` — the `tabIndex` prop's
//! mirror (`stamps::apply_focus`), implied on a text input. A focused
//! `<button>` clicks on Enter and Space ([`click_focused_button`]).
//!
//! A press focuses **the topmost pressed entity that has a focusable (a
//! `TabIndex`) on itself or an ancestor**; if nothing pressed resolves, the
//! focus clears. It is the click rule (`collect_ui_events`) applied to focus,
//! and it follows the pass-through model: a decorative overlay over a field
//! does not eat the focus, the innermost of two nested focusables wins, and
//! a label resolves to the control around it.
//!
//! Bevy's own pointer focus (`PointerFocusPlugin`) cannot give that under
//! pass-through stacks: it asks for focus once per *pressed entity*, in
//! hover-map (hash) order, and every request that reaches the window clears
//! the focus. One click on a pass-through focusable also presses its
//! ancestors and the window, so the node ends up focused or not by hash
//! order; one click through two pointers (the mouse, and the pointer
//! remapped into a 3D-transformed layer or a `<surface>`) blurs and
//! refocuses a focused field every time — and bevy's text input collapses
//! its selection on `FocusLost`. So [`resolve_pointer_focus`] replaces what
//! bevy recorded for a press that touched React UI.

use bevy::ecs::entity::Entities;
use bevy::input::ButtonState;
use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{FocusCause, FocusedInput, InputFocus, InputFocusVisible};
use bevy::picking::PickingSystems;
use bevy::picking::events::PointerPress;
use bevy::picking::pointer::{PointerButton, PointerId};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::InteractionDisabled;

use super::events::climb;
use crate::bridge::{FocusState, JsBridge, OutboundResource, ReactNode};
use crate::ext::VirtualPointers;
use crate::layer::pick3d::Transform3dPointer;
use crate::protocol::outbound::{Outbound, UiEvent};
use crate::protocol::props::Props;

/// `InputFocus` as it was before this frame's picking events ran.
#[derive(Resource, Default)]
pub(crate) struct FocusBeforePress(InputFocus);

/// Schedule the press rule around bevy_picking's event pass.
pub(crate) fn add_press_rule(app: &mut App) {
    app.init_resource::<FocusBeforePress>().add_systems(
        PreUpdate,
        (
            snapshot_focus.before(PickingSystems::Hover),
            // The ordering edge is also the sync point that runs the picking
            // observers (bevy's pointer focus among them).
            resolve_pointer_focus.after(PickingSystems::Hover),
        ),
    );
}

fn snapshot_focus(focus: Option<Res<InputFocus>>, mut before: ResMut<FocusBeforePress>) {
    if let Some(focus) = focus {
        before.0.clone_from(&focus);
    }
}

/// Apply the press rule (module docs) to this frame's presses. Presses that
/// touched no React node are left to bevy (the app's own widgets, a click on
/// the bare window).
///
/// Costs nothing without a press; on one, the pressed stack × its depth.
#[allow(clippy::too_many_arguments)]
fn resolve_pointer_focus(
    mut presses: MessageReader<PointerPress>,
    before: Res<FocusBeforePress>,
    focus: Option<ResMut<InputFocus>>,
    react: Query<(), With<ReactNode>>,
    focusable: Query<(), With<TabIndex>>,
    child_of: Query<&ChildOf>,
    virtual_pointers: Option<Res<VirtualPointers>>,
    transform3d: Option<Res<Transform3dPointer>>,
) {
    /// One pointer's press: whether it touched React UI, and the topmost
    /// pressed entity that resolves to a focusable (depth, target, button).
    struct Press {
        pointer: PointerId,
        react: bool,
        // ponytail: depth alone, like the click rule — two UI cameras
        // overlapping on one window would need the hit's camera order too.
        best: Option<(f32, Entity, PointerButton)>,
    }
    let mut pressed: Vec<Press> = Vec::new();
    for ev in presses.read() {
        let index = pressed
            .iter()
            .position(|p| p.pointer == ev.pointer.id)
            .unwrap_or_else(|| {
                pressed.push(Press {
                    pointer: ev.pointer.id,
                    react: false,
                    best: None,
                });
                pressed.len() - 1
            });
        let press = &mut pressed[index];
        press.react |= react.contains(ev.entity);
        if press.best.is_none_or(|(depth, ..)| ev.hit.depth < depth)
            && let Some(target) = climb(ev.entity, &child_of, |e| focusable.contains(e))
        {
            press.best = Some((ev.hit.depth, target, ev.button));
        }
    }
    if !pressed.iter().any(|p| p.react) {
        return;
    }
    let Some(mut focus) = focus else {
        return;
    };
    // One physical click can press through two pointers. They are asked in
    // the order the user sees their UI: a 3D-transformed layer is drawn over
    // the window's layout, a `<surface>` in the world behind it.
    pressed.sort_by_key(|p| {
        if transform3d.as_ref().is_some_and(|t| t.id == p.pointer) {
            0
        } else if virtual_pointers
            .as_ref()
            .is_some_and(|v| v.contains(p.pointer))
        {
            2
        } else {
            1
        }
    });
    let target = pressed.iter().find_map(|p| p.best);
    // Drop what bevy's pointer focus recorded for this press (every other
    // focus write made while the picking events ran goes with it), then
    // record the one real change, if any. A press on the focused element
    // records nothing at all.
    if *focus != before.0 {
        *focus = before.0.clone();
    }
    match target {
        Some((_, entity, button)) if focus.get() != Some(entity) => focus.set(
            entity,
            if button == PointerButton::Primary {
                FocusCause::Pressed
            } else {
                FocusCause::Navigated
            },
        ),
        None if focus.get().is_some() => focus.clear(),
        _ => {}
    }
}

/// Mirror bevy's `InputFocus` onto React elements: [`FocusState`] (the
/// `focusStyle` overlay and its indicator bit) and the `focus`/`blur`
/// events. A diff of the resource against last frame's rather than bevy's
/// `FocusGained`/`FocusLost` (which replay every recorded change, bubbling,
/// in `PostUpdate`): at most one `blur` and one `focus` per frame, blur
/// first, and before the restyle — the style lands in the frame the focus
/// moved.
///
/// A despawned element gives the focus up here. Bevy itself only notices on
/// the next key press, and until then names a dead entity (bevy_winit skips
/// its accessibility updates meanwhile); no `blur` is sent — the element's
/// handlers went with it. So does an element that became `disabled` while
/// focused (bevy's `InteractionDisabled` alone keeps the focus).
///
/// A few lookups per frame, whatever the node count.
#[allow(clippy::too_many_arguments)]
pub fn sync_focus(
    // `Option`s: headless harnesses run this without bevy's focus plugin or
    // the bridge.
    focus: Option<ResMut<InputFocus>>,
    indicator: Option<Res<InputFocusVisible>>,
    out: Option<Res<OutboundResource>>,
    bridge: Option<Res<JsBridge>>,
    entities: &Entities,
    nodes: Query<(&ReactNode, Has<InteractionDisabled>)>,
    inputs: Query<(), With<EditableText>>,
    mut states: Query<&mut FocusState>,
    mut last: Local<Option<Entity>>,
) {
    let Some(mut focus) = focus else {
        return;
    };
    let mut current = focus.get();
    if current.is_some_and(|entity| {
        !entities.contains(entity) || nodes.get(entity).is_ok_and(|(_, disabled)| disabled)
    }) {
        focus.clear();
        current = None;
    }
    // Every frame, not only on a change: a `focusStyle` can arrive while the
    // element is focused, and the indicator can turn while the focus stays.
    if let Some(entity) = current
        && let Ok(mut state) = states.get_mut(entity)
    {
        // A text input shows its indicator however it was focused (CSS
        // `:focus-visible`); anything else only under keyboard navigation.
        let visible = indicator.is_some_and(|i| i.0) || inputs.contains(entity);
        state.set_if_neq(FocusState {
            focused: true,
            visible,
        });
    }
    let old = std::mem::replace(&mut *last, current);
    if old == current {
        return;
    }
    let send = |entity: Entity, event: &str, wanted: fn(&Props) -> bool| {
        let (Ok((node, _)), Some(out), Some(bridge)) = (nodes.get(entity), &out, &bridge) else {
            return;
        };
        if bridge.props_cache.get(&node.0).is_some_and(|p| wanted(p)) {
            let _ = out.0.send(Outbound::ElementEvent {
                id: node.0,
                event: event.to_owned(),
                payload: serde_json::Value::Null,
            });
        }
    };
    if let Some(old) = old {
        if let Ok(mut state) = states.get_mut(old) {
            state.set_if_neq(FocusState::default());
        }
        send(old, "blur", |props| props.on_blur);
    }
    if let Some(new) = current {
        send(new, "focus", |props| props.on_focus);
    }
}

/// A focused `<button>` clicks on Enter or Space: the key going down, not
/// its repeats (the rule of bevy's widget `Button`). Bevy's own `Activate`
/// is not forwarded instead — it also fires for pointer clicks, which
/// `collect_ui_events` already reports.
pub(crate) fn click_focused_button(
    key: On<FocusedInput<KeyboardInput>>,
    buttons: Query<&ReactNode, (With<bevy::ui_widgets::Button>, Without<InteractionDisabled>)>,
    out: Option<Res<OutboundResource>>,
) {
    let input = &key.input;
    // The event bubbles up to the window: act where it started.
    if key.focused_entity != key.original_event_target()
        || input.repeat
        || input.state != ButtonState::Pressed
        || !matches!(input.key_code, KeyCode::Enter | KeyCode::Space)
    {
        return;
    }
    if let (Ok(node), Some(out)) = (buttons.get(key.focused_entity), out) {
        let _ = out.0.send(Outbound::UiEvent {
            event: UiEvent {
                id: node.0,
                kind: "click".to_owned(),
                ..default()
            },
        });
    }
}

#[cfg(test)]
mod sync_tests;
#[cfg(test)]
mod tests;
