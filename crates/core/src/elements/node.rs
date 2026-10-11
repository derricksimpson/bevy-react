//! `<node>` (the plain styled container), `<button>` (a node that captures
//! the pointer by default and carries bevy's widget `Button`), and `<root>` (a
//! detached, window-filling overlay root).

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::element::{Common, Element};
use crate::ext::ElementFlags;
use crate::protocol::animatable::Animatable;
use crate::protocol::units::Length;
use crate::style::Style;
use crate::style::props::{FLEX_DIRECTION, FOCUS_POLICY, GLOBAL_Z_INDEX, HEIGHT, WIDTH};

/// `<node>`: a plain styled container.
pub static NODE: Element = Element::new("node");

/// `<button>`: a node carrying bevy's headless `ui_widgets::Button` (plus the
/// `Interaction` the hover/press styling rides) that captures the pointer by
/// default — like bevy_ui's native buttons, so a button doesn't leak its click
/// to a sibling, an ancestor, or the 3D scene behind it.
pub static BUTTON: Element = Element {
    default_style: Some(button_style),
    // The headless widget button (`Role::Button` for accessibility). bevy_ui's
    // old `Button` (0.20-deprecated, now private) also required `Interaction`,
    // which the hover/press styling still rides — so it is inserted here.
    spawn: Some(|ctx| ctx.spawn((bevy::ui_widgets::Button, Interaction::default()))),
    ..Element::new("button")
};

fn button_style() -> Style {
    let mut style = Style::default();
    style.set(&FOCUS_POLICY, FocusPolicy::Block);
    style
}

/// Label each `<button>` for assistive technology with the text it shows:
/// every text block in its subtree, in tree order, joined by spaces. (The
/// headless `ui_widgets::Button` only sets the role; bevy_ui's old `Button`
/// labelled itself from its child text.) Recomputed when the button mounts
/// and when its direct children change — not on a text edit deeper down.
#[allow(clippy::type_complexity)]
pub fn label_buttons(
    mut commands: Commands,
    buttons: Query<
        Entity,
        (
            With<bevy::ui_widgets::Button>,
            With<crate::bridge::ReactNode>,
            Or<(Added<bevy::ui_widgets::Button>, Changed<Children>)>,
        ),
    >,
    children: Query<&Children>,
    texts: Query<(), With<Text>>,
    mut text_reader: bevy::ui::widget::TextUiReader,
) {
    for button in &buttons {
        let label = children
            .iter_descendants_depth_first(button)
            .filter(|e| texts.contains(*e))
            .flat_map(|block| {
                text_reader
                    .iter(block)
                    .filter_map(|(_, _, item)| match item {
                        bevy::text::TextElement::Text { text, .. } => Some(text.to_string()),
                        bevy::text::TextElement::Box(_) => None,
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if label.is_empty() {
            commands.entity(button).remove::<AccessibleLabel>();
        } else {
            commands.entity(button).insert(AccessibleLabel::new(label));
        }
    }
}

/// `<root>`: the screen-space twin of `<surface>` — a styled container that
/// is a **detached UI root** on the default UI camera, for overlays that
/// must float above and stay out of the app's own tree (the devtools panel).
/// It never blocks or hovers picking itself; its children are ordinary
/// pickable nodes.
pub static ROOT: Element = Element {
    flags: ElementFlags {
        pick_ignore: true,
        ..ElementFlags::DETACHED_ROOT
    },
    common: Common::IDENTITY,
    default_style: Some(root_style),
    spawn: Some(|ctx| ctx.spawn(crate::bridge::RRoot)),
    ..Element::new("root")
};

/// The default style of a `<root>`: a window-filling overlay just above the
/// window tree. `globalZIndex: 1` (not a magic max) because bevy_ui sorts
/// root nodes by `(GlobalZIndex, ZIndex)` and breaks a tie by history: a
/// root that is new this frame, or whose z-index component was just written,
/// goes above its equals, the others keep last frame's order, and two that
/// arrive on the same frame fall back to query iteration order (unspecified).
/// So a bare `<root>` at the window tree's implicit 0 would usually land
/// above it — by mount timing, not by rule. `1` is above by rule, while
/// leaving the whole range open for the user's own layering
/// (`style.globalZIndex` overrides in either direction; the devtools panel
/// claims `i32::MAX` explicitly).
///
/// It defaults to a column, like the main UI root: bevy's own default is
/// `row`, but a row container mis-measures a single content-sized child that
/// has `maxWidth` + wrapping text (sized at max-content during the row's
/// main-axis pass, then clamped and wrapped — its height committed one line
/// short).
///
/// It fills its camera's viewport in viewport units (`100vw` × `100vh`), not
/// `100%`: bevy_ui lays a root out as an item of an implicit grid whose auto
/// row grows to the root's content, so a `100%` root would outgrow the window
/// as soon as its content is taller.
fn root_style() -> Style {
    let mut style = Style::default();
    style.set(&WIDTH, Animatable::Static(Length::Vw(100.0)));
    style.set(&HEIGHT, Animatable::Static(Length::Vh(100.0)));
    style.set(&FLEX_DIRECTION, FlexDirection::Column);
    style.set(&GLOBAL_Z_INDEX, 1);
    style
}
