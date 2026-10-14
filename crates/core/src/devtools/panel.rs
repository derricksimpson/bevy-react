//! Panel chrome lifecycle: the toggle key, JS↔Bevy open/close sync, and the
//! docked panel's window-space reservation.

use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;

use crate::event::ReactEvents;
use crate::protocol::NodeId;
use crate::request::Request;
use crate::window::{ViewportInset, WindowSize, ui_viewport_size};
use crate::{react_event, react_message, react_request};

use super::{DevtoolsConfig, DevtoolsState, DockSide};

/// Bevy → JS: the panel's open state changed Bevy-side (toggle key / auto-open).
/// Carries the resulting state (not a bare "flip") so the panel mirrors Bevy
/// instead of tracking parity.
#[react_event(name = "devtools.toggle")]
pub(super) struct DevtoolsToggle {
    pub(super) open: bool,
}

/// JS → Bevy: the panel opened or closed itself (close button, install sync).
#[react_message(name = "devtools.open")]
pub(super) struct DevtoolsOpenMessage {
    pub(super) open: bool,
}

/// JS → Bevy: the panel's "overlay" toggle — show/hide the persistent
/// selected-node box.
#[react_message(name = "devtools.overlay")]
pub(super) struct DevtoolsOverlayMessage {
    on: bool,
}

/// JS → Bevy: the panel's own `<root>` node id (`None` when the panel closes).
/// Sent on open so [`super::pick::drive_pick_mode`] can reject exactly the
/// panel.
#[react_message(name = "devtools.panelRoot")]
pub(super) struct DevtoolsPanelRootMessage {
    id: Option<NodeId>,
}

/// JS → Bevy: the panel's effective space reservation. `side: None` = no
/// reservation (the reserve toggle is off, the panel floats, or it closed);
/// otherwise the app UI is pushed off that edge by `width` logical pixels
/// (see [`apply_dock_reservation`]).
#[react_message(name = "devtools.dock")]
pub(super) struct DevtoolsDockMessage {
    side: Option<String>,
    width: f32,
}

/// JS → Bevy: the whole UI viewport, which the panel lays out against — the
/// built-in `window.size` reports what a pushing panel leaves the app (unit
/// payload; rejects without a viewport).
#[react_request(name = "devtools.windowSize", response = WindowSize)]
pub(super) struct DevtoolsWindowSizeGet;

pub(super) fn on_window_size_request(
    req: On<Request<DevtoolsWindowSizeGet>>,
    cameras: Query<&Camera, With<IsDefaultUiCamera>>,
    windows: Query<&Window>,
) {
    match ui_viewport_size(&cameras, &windows) {
        Some(size) => req.respond(WindowSize {
            width: size.x,
            height: size.y,
        }),
        None => req.respond_err("no UI viewport available"),
    }
}

/// JS → Bevy: the configured keys, for the panel's tooltips (unit payload).
#[react_request(name = "devtools.keys", response = DevtoolsKeys)]
pub(super) struct DevtoolsKeysGet;

/// [`DevtoolsConfig`]'s keys by their printed names (`"F12"`, `"P"`).
#[derive(serde::Serialize, ts_rs::TS)]
pub(super) struct DevtoolsKeys {
    toggle: String,
    pause: String,
}

pub(super) fn on_keys_request(req: On<Request<DevtoolsKeysGet>>, cfg: Res<DevtoolsConfig>) {
    req.respond(DevtoolsKeys {
        toggle: key_name(cfg.toggle_key),
        pause: key_name(cfg.pause_key),
    });
}

/// A key as a person reads it: the `KeyCode` name without the `Key`/`Digit`
/// prefix of letters and digits (`KeyP` → `P`).
fn key_name(key: KeyCode) -> String {
    let name = format!("{key:?}");
    let short = name.strip_prefix("Key").or(name.strip_prefix("Digit"));
    short.unwrap_or(&name).to_owned()
}

pub(super) fn on_open_message(msg: On<DevtoolsOpenMessage>, mut state: ResMut<DevtoolsState>) {
    state.open = msg.event().open;
    if !state.open {
        exit_interactions(&mut state);
    }
}

pub(super) fn on_overlay_message(
    msg: On<DevtoolsOverlayMessage>,
    mut state: ResMut<DevtoolsState>,
) {
    state.show_selection_overlay = msg.event().on;
}

pub(super) fn on_panel_root_message(
    msg: On<DevtoolsPanelRootMessage>,
    mut state: ResMut<DevtoolsState>,
) {
    state.panel_root = msg.event().id;
}

pub(super) fn on_dock_message(msg: On<DevtoolsDockMessage>, mut state: ResMut<DevtoolsState>) {
    state.dock_side = match msg.event().side.as_deref() {
        Some("left") => Some(DockSide::Left),
        Some("right") => Some(DockSide::Right),
        _ => None,
    };
    state.dock_width = msg.event().width.max(0.0);
}

/// Clear the transient interaction state that shouldn't outlive a closed panel.
fn exit_interactions(state: &mut DevtoolsState) {
    state.pick = false;
    state.tree_hover = None;
    state.pick_hover = None;
    // Belt-and-braces: the JS panel also reports `layersOpen: false` /
    // `consoleOpen: false` / `animationsOpen: false` when those tabs unmount,
    // but every close path must kill the streams even if the message is lost.
    state.layers_tab_open = false;
    state.console_tab_open = false;
    state.animations_tab_open = false;
    state.console_last_seq = None;
}

/// Flip the panel on the configured key and tell the JS panel the new state.
pub(super) fn toggle_on_key(
    keys: Res<ButtonInput<KeyCode>>,
    cfg: Res<DevtoolsConfig>,
    mut state: ResMut<DevtoolsState>,
    events: ReactEvents,
) {
    if !keys.just_pressed(cfg.toggle_key) {
        return;
    }
    state.open = !state.open;
    if !state.open {
        exit_interactions(&mut state);
    }
    events.send(&DevtoolsToggle { open: state.open });
}

/// Reserve window space for a docked panel: narrow the app's [`UiRoot`] by
/// the panel's width and inset it from the reserved edge, so the whole
/// reconciler tree reflows beside the panel (the panel pushes the app aside
/// rather than overlapping it), and release it whenever the reservation ends.
/// The root is `100vw` wide (`plugin.rs`), so a margin alone would only shift
/// it. The [`ViewportInset`] tells the app too (`window.size`, `resize`), for
/// layouts computed from the window's size — as a docked browser devtools
/// shrinks the page's viewport. Gated on `state.open`, so every close path
/// (close button, toggle key) releases the space with no extra bookkeeping.
///
/// Compared before writing — an unconditional `Node` deref-mut would re-run
/// app layout every frame. The reserved width is clamped against the UI
/// viewport (the one the panel lays out against) so a huge panel can't push
/// the app entirely off-screen; without a viewport nothing is reserved.
///
/// Known limitation: app-created `<root>` overlays are detached full-window
/// trees (see `reconcile.rs` `root_base`), so they are not pushed — only the
/// main tree under [`UiRoot`] is.
///
/// [`UiRoot`]: crate::plugin::UiRoot
pub(super) fn apply_dock_reservation(
    state: Res<DevtoolsState>,
    cameras: Query<&Camera, With<IsDefaultUiCamera>>,
    windows: Query<&Window>,
    mut inset: ResMut<ViewportInset>,
    mut root: Query<&mut Node, With<crate::plugin::UiRoot>>,
) {
    let Ok(mut node) = root.single_mut() else {
        return;
    };
    let viewport = ui_viewport_size(&cameras, &windows)
        .map(|size| size.x)
        .filter(|w| *w > 0.0);
    let (side, width) = match (state.open.then_some(state.dock_side).flatten(), viewport) {
        (Some(side), Some(vw)) => (Some(side), state.dock_width.min(vw - 100.0).max(0.0)),
        _ => (None, 0.0),
    };
    let (left, right) = match side {
        Some(DockSide::Left) => (Val::Px(width), Val::ZERO),
        Some(DockSide::Right) => (Val::ZERO, Val::Px(width)),
        None => (Val::ZERO, Val::ZERO),
    };
    let margin = UiRect {
        left,
        right,
        ..node.margin
    };
    let root_width = Val::Vw(viewport.map_or(100.0, |vw| 100.0 * (1.0 - width / vw)));
    if node.margin != margin || node.width != root_width {
        node.margin = margin;
        node.width = root_width;
    }
    inset.set_if_neq(ViewportInset(width));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devtools::test_util::{drain_events, test_app};

    #[test]
    fn toggle_key_flips_state_and_notifies_js() {
        let (mut app, mut rx) = test_app(DevtoolsConfig {
            toggle_key: KeyCode::F9,
            ..default()
        });
        app.update();
        assert!(drain_events(&mut rx).is_empty(), "no toggle before the key");

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();
        let events = drain_events(&mut rx);
        assert_eq!(
            events
                .iter()
                .find(|(name, _)| name == "devtools.toggle")
                .map(|(_, v)| v["open"].as_bool()),
            Some(Some(true)),
            "the configured key must open the panel and notify JS"
        );
        assert!(app.world().resource::<DevtoolsState>().open);

        // Release + press again closes it.
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear_just_pressed(KeyCode::F9);
            keys.release(KeyCode::F9);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update();
        let events = drain_events(&mut rx);
        assert_eq!(
            events
                .iter()
                .find(|(name, _)| name == "devtools.toggle")
                .map(|(_, v)| v["open"].as_bool()),
            Some(Some(false)),
            "pressing again must close the panel"
        );
        assert!(!app.world().resource::<DevtoolsState>().open);
    }

    /// An open panel with a dock side narrows the app root by its width and
    /// insets it from that edge; flipping sides swaps the inset; closing
    /// releases it. The harness window is 1280 logical px wide.
    #[test]
    fn dock_reservation_narrows_and_insets_uiroot() {
        let (mut app, _rx) = test_app(DevtoolsConfig::default());
        app.world_mut().spawn(Window::default());
        let root = app
            .world_mut()
            .spawn((
                Node {
                    width: Val::Vw(100.0),
                    ..default()
                },
                crate::plugin::UiRoot,
            ))
            .id();
        let layout = |app: &mut App| {
            let node = app.world().entity(root).get::<Node>().unwrap();
            (node.margin.left, node.margin.right, node.width)
        };

        {
            let mut state = app.world_mut().resource_mut::<DevtoolsState>();
            state.open = true;
            state.dock_side = Some(DockSide::Right);
            state.dock_width = 320.0;
        }
        app.update();
        assert_eq!(layout(&mut app), (Val::ZERO, Val::Px(320.0), Val::Vw(75.0)));
        assert_eq!(
            *app.world().resource::<ViewportInset>(),
            ViewportInset(320.0)
        );

        app.world_mut().resource_mut::<DevtoolsState>().dock_side = Some(DockSide::Left);
        app.update();
        assert_eq!(layout(&mut app), (Val::Px(320.0), Val::ZERO, Val::Vw(75.0)));

        // Any close path just flips `open`; the reservation releases for free.
        app.world_mut().resource_mut::<DevtoolsState>().open = false;
        app.update();
        assert_eq!(layout(&mut app), (Val::ZERO, Val::ZERO, Val::Vw(100.0)));
        assert_eq!(*app.world().resource::<ViewportInset>(), ViewportInset(0.0));
    }

    #[test]
    fn key_names_drop_the_letter_and_digit_prefixes() {
        assert_eq!(key_name(KeyCode::F12), "F12");
        assert_eq!(key_name(KeyCode::KeyP), "P");
        assert_eq!(key_name(KeyCode::Digit1), "1");
    }

    /// The `devtools.dock` message maps its loose wire shape onto the state:
    /// known sides parse, anything else (or `None`) clears the reservation,
    /// and a negative width clamps to zero.
    #[test]
    fn dock_message_parses_side_and_clamps_width() {
        let (mut app, _rx) = test_app(DevtoolsConfig::default());
        let dock = |app: &mut App, side: Option<&str>, width: f32| {
            app.world_mut().trigger(DevtoolsDockMessage {
                side: side.map(String::from),
                width,
            });
            let state = app.world().resource::<DevtoolsState>();
            (state.dock_side, state.dock_width)
        };

        assert_eq!(
            dock(&mut app, Some("left"), 320.0),
            (Some(DockSide::Left), 320.0)
        );
        assert_eq!(
            dock(&mut app, Some("right"), 280.0),
            (Some(DockSide::Right), 280.0)
        );
        assert_eq!(dock(&mut app, Some("bogus"), -5.0), (None, 0.0));
        assert_eq!(dock(&mut app, None, 380.0), (None, 380.0));
    }
}
