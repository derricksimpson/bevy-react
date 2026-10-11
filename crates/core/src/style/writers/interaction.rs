//! Interaction writers: pointer capture and the cursor.

use bevy::picking::Pickable;
use bevy::ui::FocusPolicy;

use crate::cursor::NodeCursor;
use crate::style::props::*;
use crate::style::{Writer, owns};
use crate::ui_map::remove_unless_fresh;

/// `focusPolicy` → `Pickable` (unset passes — an element that captures the
/// pointer by default, like `<button>`, says so in its default style):
/// hover, press, drags and clicks all ride bevy_picking, whose backend and
/// hover map stop at the first blocking `Pickable` — and block when it is
/// absent, so it is always inserted, never removed. A `<root>` never blocks
/// or hovers picking itself (its children are ordinary pickable nodes).
///
/// `FocusPolicy` is still written too: bevy's legacy `ui_focus_system` keeps
/// running for the app's own bevy_ui nodes, and the policy is what stops
/// their `Interaction` beneath a blocking React node. (Goes when Bevy
/// removes that system.)
pub static FOCUS_POLICY_WRITER: Writer = Writer {
    reads: &[&FOCUS_POLICY],
    attrs: &[],
    writes: &[owns::<FocusPolicy>, owns::<Pickable>],
    apply: |ctx, s, ec| {
        let policy = s.get(&FOCUS_POLICY).copied().unwrap_or(FocusPolicy::Pass);
        ec.insert(policy);
        ec.insert(focus_pickable(ctx.flags, policy));
    },
};

/// The `Pickable` mirror of `policy` on an element with `flags`.
///
/// A text input blocks the picks beneath it whatever its policy: bevy's
/// pointer focus (`PointerFocusPlugin`) asks for focus once per PRESSED
/// entity, and a request that reaches the window clears the focus. Under a
/// pass-through input one press would also land on every node below it and
/// on the window, so the same click would both focus and blur the field (in
/// hover-map order). Blocking keeps the press, and its one focus request, on
/// the input — so the pointer over an input hovers and presses nothing
/// beneath it either.
pub(crate) fn focus_pickable(flags: crate::ext::ElementFlags, policy: FocusPolicy) -> Pickable {
    if flags.pick_ignore {
        Pickable::IGNORE
    } else {
        Pickable {
            should_block_lower: policy == FocusPolicy::Block
                || flags.text == crate::ext::TextRole::Input,
            is_hoverable: true,
        }
    }
}

/// `cursor` → `NodeCursor`, which `drive_cursor_icon` writes onto the
/// window's `CursorIcon` on hover.
pub static CURSOR_WRITER: Writer = Writer {
    reads: &[&CURSOR],
    attrs: &[],
    writes: &[owns::<NodeCursor>],
    apply: |ctx, s, ec| match s.get(&CURSOR) {
        Some(c) => {
            ec.insert(NodeCursor(c.clone()));
        }
        None => remove_unless_fresh::<NodeCursor>(ec, ctx.fresh),
    },
};
