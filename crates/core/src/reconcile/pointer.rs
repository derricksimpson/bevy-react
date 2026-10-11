//! Pointer/drag reporting for elements that declared `onPointer*` handlers,
//! for every pointer: the cross-frame drag state ([`ActiveDrag`]) and the
//! per-frame drag driver. Hover boundary events live in [`super::hover`];
//! discrete clicks in [`super::events`].

use bevy::picking::events::PointerPress;
use bevy::picking::pointer::{
    Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
};
use bevy::prelude::*;

use super::events::{Modifiers, dom_button, send_ui_event};
use super::picking::{Geometry, client_pos, device, event_pos, sample_pointer};
use crate::bridge::{JsBridge, PointerHandlers, ReactNode};
use crate::ext::{EventLocalPos, VirtualPointers};
use crate::layer::pick3d::Transform3dPointer;

/// One element being dragged: an `onPointer*` element a pointer pressed, any
/// button, until that button is released.
pub(crate) struct Drag {
    pub entity: Entity,
    /// The pointer whose location and inputs the drag follows.
    pub pointer: PointerId,
    /// The button that began it — the one whose release ends it; any other
    /// button pressed meanwhile is ignored.
    pub button: PointerButton,
    /// The last positions reported (`x`/`y` and `clientX`/`clientY`): what a
    /// move is measured against, and the fallback once the pointer is gone.
    last_pos: Vec2,
    last_abs: Vec2,
}

/// The drags in progress — at most one per device: the mouse (itself or its
/// transform3d remap), each finger, each virtual pointer. A `Resource` so the
/// touch-scroll path (`crate::touch_scroll`) can see which touch a drag owns
/// — that touch is *contested* there, and a pan along a scrollable axis
/// steals it — and the click collector which element a gesture belongs to.
#[derive(Resource, Default)]
pub(crate) struct ActiveDrag {
    drags: Vec<Drag>,
}

impl ActiveDrag {
    pub(crate) fn begin(
        &mut self,
        entity: Entity,
        pointer: PointerId,
        button: PointerButton,
        pos: Vec2,
        abs: Vec2,
    ) {
        self.drags.push(Drag {
            entity,
            pointer,
            button,
            last_pos: pos,
            last_abs: abs,
        });
    }

    /// The drag `device` (see [`device`]) is running.
    pub(crate) fn of_device(
        &self,
        of: PointerId,
        transform3d: Option<&Transform3dPointer>,
    ) -> Option<&Drag> {
        self.drags
            .iter()
            .find(|drag| device(drag.pointer, transform3d) == of)
    }

    /// Whether touch `id` is dragging an element.
    pub(crate) fn owns_touch(&self, id: u64) -> bool {
        self.drags
            .iter()
            .any(|drag| drag.pointer == PointerId::Touch(id))
    }
}

/// Drive `pointerDown` / `pointerMove` / `pointerUp` for elements that
/// declared `onPointer*` handlers, from bevy_picking — one path for the
/// mouse, each touch, the transform3d pointer and a feature's virtual
/// pointers. Unlike the discrete click path, a drag follows its pointer
/// across frames, so a dragged control (a slider) keeps updating once the
/// pointer leaves its bounds: `x`/`y` are the pointer's position within the
/// element, clamped to `0..1`.
///
/// - **Begin**: a [`PointerPress`] on an element with handlers, on a device
///   without a drag. bevy_picking presses everything in the hover map, so
///   the element is under the pointer and nothing above it blocks. When
///   several qualify, the transform3d pointer's press beats the raw mouse's
///   (the quad is what the user sees), then the topmost (smallest
///   `HitData.depth`) wins — the rule click ownership follows.
/// - **Follow**: the pointer's location each frame; `pointerMove` only when
///   it moved (DOM semantics), not once per held frame.
/// - **End**: that pointer's release of the button, or its cancel →
///   `pointerUp` where the input says it happened. A touch the scroll
///   arbitration stole (`TouchScrollState::stole_drag`) → `pointerLeave`
///   instead (the DOM `pointercancel` analog: the finger went to the scroll,
///   and its later release is nobody's drag).
///
/// Costs this frame's press and button inputs plus the live drags — never
/// the node count.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn collect_pointer_events(
    bridge: Res<JsBridge>,
    mut presses: MessageReader<PointerPress>,
    mut inputs: MessageReader<PointerInput>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    cameras: Query<&Camera>,
    transform3d: Option<Res<Transform3dPointer>>,
    virtual_pointers: Option<Res<VirtualPointers>>,
    nodes: Query<(
        &ReactNode,
        &PointerHandlers,
        // SVG shapes report x/y in user units instead (see [`event_pos`]).
        Option<&EventLocalPos>,
        Geometry,
    )>,
    mut capture: ResMut<crate::PointerCapture>,
    mut active: ResMut<ActiveDrag>,
    // Written by `apply_touch_scroll` (ordered after, same set): a steal
    // recorded in frame N ends the drag here in frame N+1.
    touch_scroll: Res<crate::touch_scroll::TouchScrollState>,
    // `Option` so headless harnesses without input needn't init it.
    keys: Option<Res<ButtonInput<KeyCode>>>,
) {
    let mods = Modifiers::read(keys.as_deref());
    let transform3d = transform3d.as_deref();
    let emit = |rnode: &ReactNode, kind: &str, pos: Vec2, abs: Vec2, button: PointerButton| {
        send_ui_event(
            &bridge,
            rnode.0,
            kind,
            Some(pos),
            Some(abs),
            Some(dom_button(button)),
            Some(mods),
        );
    };

    // Begin: per idle device, the best of this frame's presses on handler
    // elements. A pass-through stack presses SEVERAL of them at once (a
    // drawing canvas inside a draggable dialog); message order means nothing.
    let live = active.drags.len();
    let mut begins: Vec<&PointerPress> = Vec::new();
    for press in presses.read() {
        let of = device(press.pointer.id, transform3d);
        if !nodes.contains(press.entity) || active.of_device(of, transform3d).is_some() {
            continue;
        }
        // The remapped pointer first (`false` sorts first), then the topmost.
        let rank = |press: &PointerPress| (press.pointer.id == of, press.hit.depth);
        match begins
            .iter_mut()
            .find(|best| device(best.pointer.id, transform3d) == of)
        {
            Some(best) if rank(press) < rank(best) => *best = press,
            Some(_) => {}
            None => begins.push(press),
        }
    }
    for press in begins {
        let Ok((rnode, handlers, user, geometry)) = nodes.get(press.entity) else {
            continue;
        };
        let location = press.pointer.location();
        let pos = event_pos(user, geometry, &cameras, &location).unwrap_or(Vec2::ZERO);
        let abs = client_pos(press.pointer.id, &location, &pointers, transform3d);
        active.begin(press.entity, press.pointer.id, press.button, pos, abs);
        if handlers.down {
            emit(rnode, "pointerDown", pos, abs, press.button);
        }
    }

    // Follow each drag, or end it.
    let inputs: Vec<&PointerInput> = inputs
        .read()
        .filter(|input| {
            matches!(
                input.action,
                PointerAction::Press(_) | PointerAction::Release(_) | PointerAction::Cancel
            )
        })
        .collect();
    let mut index = 0;
    active.drags.retain_mut(|drag| {
        let fresh = index >= live;
        index += 1;
        // A node despawned mid-gesture keeps its binding (and the pointer)
        // until the release, silently.
        let node = nodes.get(drag.entity).ok();
        let stolen = matches!(drag.pointer, PointerId::Touch(id) if touch_scroll.stole_drag(id));
        let released = release_of(&inputs, drag, fresh);
        let sample = sample_pointer(drag.pointer, &pointers, transform3d);
        if !stolen
            && released.is_none()
            && let Some(sample) = &sample
        {
            if let Some((rnode, handlers, user, geometry)) = node {
                let pos =
                    event_pos(user, geometry, &cameras, &sample.location).unwrap_or(drag.last_pos);
                let moved = sample.client != drag.last_abs;
                drag.last_pos = pos;
                drag.last_abs = sample.client;
                if moved && handlers.moved {
                    emit(rnode, "pointerMove", pos, sample.client, drag.button);
                }
            }
            return true;
        }
        // Where it ends: the release's own location (a virtual pointer parks
        // off-bounds right after it), else the pointer's (a stolen touch is
        // still down), else — the pointer is gone — the last reading.
        let location = released.or(sample.as_ref().map(|sample| &sample.location));
        if let Some((rnode, handlers, user, geometry)) = node {
            let (pos, abs) = match location {
                Some(location) => (
                    event_pos(user, geometry, &cameras, location).unwrap_or(drag.last_pos),
                    client_pos(drag.pointer, location, &pointers, transform3d),
                ),
                None => (drag.last_pos, drag.last_abs),
            };
            if stolen {
                if handlers.leave {
                    emit(rnode, "pointerLeave", pos, abs, drag.button);
                }
            } else if handlers.up {
                emit(rnode, "pointerUp", pos, abs, drag.button);
            }
        }
        false
    });

    // Publish whether the UI owns the pointer so world systems (e.g. a camera
    // controller) can ignore the mouse. `dragging` spans the whole gesture even
    // once the cursor leaves the element; a virtual pointer's drag is not the
    // window's. (`over_ui` is `sync_interactions`'; a wheel the UI consumes is
    // `wheel_captured`'s job.)
    capture.dragging = active.drags.iter().any(|drag| {
        !virtual_pointers
            .as_ref()
            .is_some_and(|v| v.contains(drag.pointer))
    });
    // This system is the frame's assigner; the wheel systems (`apply_scroll`,
    // `collect_wheel_events` — ordered after, same set) re-claim per frame.
    capture.wheel_captured = false;
}

/// Where the input that ends `drag` happened, among this frame's button
/// inputs: its pointer's release of its button, or its cancel. A drag begun
/// this frame only hears what follows the press that began it — a release
/// earlier in the frame ended some other gesture.
fn release_of<'a>(inputs: &[&'a PointerInput], drag: &Drag, fresh: bool) -> Option<&'a Location> {
    let mine = |input: &&&PointerInput| input.pointer_id == drag.pointer;
    let from = if fresh {
        inputs
            .iter()
            .rposition(|input| {
                mine(&input) && matches!(input.action, PointerAction::Press(b) if b == drag.button)
            })
            .map_or(0, |press| press + 1)
    } else {
        0
    };
    inputs[from..]
        .iter()
        .filter(mine)
        .find(|input| match input.action {
            PointerAction::Release(button) => button == drag.button,
            PointerAction::Cancel => true,
            _ => false,
        })
        .map(|input| &input.location)
}

#[cfg(test)]
mod tests;
