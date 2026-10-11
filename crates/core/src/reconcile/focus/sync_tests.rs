//! `tabIndex` → `TabIndex`, and [`sync_focus`]: `FocusState`, `focusStyle`
//! and the `focus`/`blur` events off bevy's `InputFocus`, through the op
//! harness.

use bevy::a11y::AccessibilityNode;
use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy::input_focus::{FocusCause, InputFocus, InputFocusVisible};
use bevy::prelude::*;
use serde_json::json;

use super::sync_focus;
use crate::bridge::{FocusState, OutboundResource};
use crate::protocol::op::Op;
use crate::protocol::outbound::Outbound;
use crate::protocol::{NodeId, ROOT_ID};
use crate::reconcile::test_util::{create, ent, op_app_with, update};
use crate::reconcile::{apply_interaction_styles, apply_js_ops};

type Outbox = tokio::sync::mpsc::UnboundedReceiver<Outbound>;

/// The op harness with bevy's focus resources, [`sync_focus`] and the
/// restyle scheduled as the plugin does, and a readable outbound channel.
fn focus_app(setup: impl FnOnce(&mut App)) -> (App, crossbeam_channel::Sender<Vec<Op>>, Outbox) {
    let (mut app, tx) = op_app_with(|app| {
        app.init_resource::<InputFocus>()
            .init_resource::<InputFocusVisible>()
            .add_systems(
                Update,
                (
                    sync_focus.after(apply_js_ops),
                    apply_interaction_styles.after(sync_focus),
                ),
            );
        setup(app);
    });
    let (out, rx) = tokio::sync::mpsc::unbounded_channel();
    app.insert_resource(OutboundResource(out));
    (app, tx, rx)
}

fn focus(app: &mut App, id: NodeId) {
    let entity = ent(app, id);
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

/// The element events sent since the last call, as `(node, event)`.
fn events(rx: &mut Outbox) -> Vec<(NodeId, String)> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|out| match out {
            Outbound::ElementEvent { id, event, .. } => Some((id, event)),
            _ => None,
        })
        .collect()
}

fn sent(pairs: &[(NodeId, &str)]) -> Vec<(NodeId, String)> {
    pairs.iter().map(|(id, e)| (*id, (*e).to_owned())).collect()
}

fn background(app: &App, id: NodeId) -> Color {
    app.world().get::<BackgroundColor>(ent(app, id)).unwrap().0
}

/// `tabIndex` lands as bevy's `TabIndex` (with an accessibility node, which
/// bevy_winit needs on the focused entity), follows a change, and goes when
/// unset; a node without it carries neither.
#[test]
fn tab_index_stamps_and_unsets() {
    let (mut app, tx, _rx) = focus_app(|_| {});
    tx.send(vec![
        create(1, "node", json!({ "tabIndex": 0 })),
        create(2, "node", json!({})),
    ])
    .unwrap();
    app.update();
    let (node, plain) = (ent(&app, 1), ent(&app, 2));
    assert_eq!(app.world().get::<TabIndex>(node), Some(&TabIndex(0)));
    assert!(app.world().get::<AccessibilityNode>(node).is_some());
    assert!(app.world().get::<TabIndex>(plain).is_none());
    assert!(app.world().get::<AccessibilityNode>(plain).is_none());

    tx.send(vec![update(1, "node", json!({ "tabIndex": -1 }), &[], &[])])
        .unwrap();
    app.update();
    assert_eq!(app.world().get::<TabIndex>(node), Some(&TabIndex(-1)));

    tx.send(vec![update(1, "node", json!({}), &["tabIndex"], &[])])
        .unwrap();
    app.update();
    assert!(app.world().get::<TabIndex>(node).is_none());
}

/// A text input is focusable without a `tabIndex` (`0`); the prop overrides,
/// and unsetting it returns to the default.
#[test]
fn an_input_is_focusable_by_default() {
    let (mut app, tx, _rx) = focus_app(|_| {});
    tx.send(vec![
        create(1, "editableText", json!({})),
        create(2, "editableText", json!({ "tabIndex": -1 })),
    ])
    .unwrap();
    app.update();
    let index = |app: &App, id| app.world().get::<TabIndex>(ent(app, id)).copied();
    assert_eq!(index(&app, 1), Some(TabIndex(0)));
    assert_eq!(index(&app, 2), Some(TabIndex(-1)));

    tx.send(vec![update(
        2,
        "editableText",
        json!({}),
        &["tabIndex"],
        &[],
    )])
    .unwrap();
    app.update();
    assert_eq!(index(&app, 2), Some(TabIndex(0)));
}

/// A `<root>` is a tab group of its own, after the window tree's.
#[test]
fn a_root_is_a_tab_group() {
    let (mut app, tx, _rx) = focus_app(|_| {});
    tx.send(vec![create(1, "root", json!({}))]).unwrap();
    app.update();
    let group = app.world().get::<TabGroup>(ent(&app, 1)).expect("a group");
    assert_eq!((group.order, group.modal), (1, false));
}

/// Moving the focus sends the old element's `blur`, then the new one's
/// `focus`, once per frame — to elements with the handler only.
#[test]
fn blur_then_focus_once_per_frame() {
    let (mut app, tx, mut rx) = focus_app(|_| {});
    let handlers = || json!({ "tabIndex": 0, "onFocus": true, "onBlur": true });
    tx.send(vec![
        create(1, "node", handlers()),
        create(2, "node", handlers()),
        create(3, "node", json!({ "tabIndex": 0 })),
    ])
    .unwrap();
    app.update();

    focus(&mut app, 1);
    app.update();
    assert_eq!(events(&mut rx), sent(&[(1, "focus")]));

    focus(&mut app, 2);
    app.update();
    assert_eq!(events(&mut rx), sent(&[(1, "blur"), (2, "focus")]));

    // Away and back within one frame: nothing happened.
    focus(&mut app, 1);
    focus(&mut app, 2);
    app.update();
    assert_eq!(events(&mut rx), []);

    focus(&mut app, 3);
    app.update();
    assert_eq!(events(&mut rx), sent(&[(2, "blur")]));

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert_eq!(events(&mut rx), [], "node 3 has no handlers");

    // A handler dropped by a re-render is no longer called.
    tx.send(vec![update(1, "node", json!({}), &["onFocus"], &[])])
        .unwrap();
    focus(&mut app, 1);
    app.update();
    assert_eq!(events(&mut rx), []);
}

/// `focusStyle` applies in the frame the focus moves, and leaves with it.
#[test]
fn focus_style_lands_the_same_frame() {
    let (mut app, tx, _rx) = focus_app(|_| {});
    tx.send(vec![create(
        1,
        "node",
        json!({
            "tabIndex": 0,
            "style": { "backgroundColor": "red" },
            "focusStyle": { "backgroundColor": "blue" },
        }),
    )])
    .unwrap();
    app.update();
    let (red, blue) = (
        crate::ui_map::parse_color("red"),
        crate::ui_map::parse_color("blue"),
    );
    assert_eq!(background(&app, 1), red);

    focus(&mut app, 1);
    app.update();
    assert_eq!(background(&app, 1), blue);

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert_eq!(background(&app, 1), red);
}

/// A `focusStyle` that arrives while the element is focused applies.
#[test]
fn a_focus_style_added_while_focused_applies() {
    let (mut app, tx, _rx) = focus_app(|_| {});
    tx.send(vec![create(1, "node", json!({ "tabIndex": 0 }))])
        .unwrap();
    app.update();
    focus(&mut app, 1);
    app.update();

    tx.send(vec![update(
        1,
        "node",
        json!({ "focusStyle": { "backgroundColor": "blue" } }),
        &[],
        &[],
    )])
    .unwrap();
    app.update();
    assert_eq!(background(&app, 1), crate::ui_map::parse_color("blue"));
}

/// An element that unmounts while focused gives the focus up at once (bevy
/// alone would keep naming the dead entity until the next key press). Its
/// `blur` is not sent: the handlers went with it.
#[test]
fn an_unmounted_focused_node_releases_the_focus() {
    let (mut app, tx, mut rx) = focus_app(|_| {});
    tx.send(vec![
        create(1, "node", json!({ "tabIndex": 0, "onBlur": true })),
        Op::Append {
            parent: ROOT_ID,
            child: 1,
        },
    ])
    .unwrap();
    app.update();
    focus(&mut app, 1);
    app.update();
    assert!(focused(&app).is_some());

    tx.send(vec![Op::Remove {
        parent: ROOT_ID,
        child: 1,
    }])
    .unwrap();
    app.update();
    assert_eq!(focused(&app), None);
    assert_eq!(events(&mut rx), []);
}

/// The indicator bit follows bevy's `InputFocusVisible` while the focus
/// stays put; a text input shows it however it was focused.
#[test]
fn the_indicator_follows_input_focus_visible() {
    let (mut app, tx, _rx) = focus_app(|_| {});
    let styled = || json!({ "tabIndex": 0, "focusStyle": { "backgroundColor": "blue" } });
    tx.send(vec![
        create(1, "node", styled()),
        create(2, "editableText", styled()),
    ])
    .unwrap();
    app.update();
    let state = |app: &App, id| *app.world().get::<FocusState>(ent(app, id)).unwrap();
    let on = |visible| FocusState {
        focused: true,
        visible,
    };

    focus(&mut app, 1);
    app.update();
    assert_eq!(state(&app, 1), on(false));

    app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
    app.update();
    assert_eq!(state(&app, 1), on(true));

    app.world_mut().resource_mut::<InputFocusVisible>().0 = false;
    app.update();
    assert_eq!(state(&app, 1), on(false));

    focus(&mut app, 2);
    app.update();
    assert_eq!(state(&app, 1), FocusState::default());
    assert_eq!(state(&app, 2), on(true));
}

/// With bevy's `TabNavigationPlugin` (the app's opt-in) Tab walks the
/// focusables under a tab group in tree order, skipping negative indices,
/// and turns the focus indicator on.
#[test]
fn tab_walks_the_focusables() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::{ButtonState, InputPlugin};
    use bevy::input_focus::InputDispatchPlugin;
    use bevy::input_focus::tab_navigation::TabNavigationPlugin;
    use bevy::window::PrimaryWindow;

    let (mut app, tx, mut rx) = focus_app(|app| {
        app.add_plugins((InputPlugin, InputDispatchPlugin, TabNavigationPlugin))
            // What the key dispatch reads in a full app.
            .add_message::<bevy::input::gamepad::GamepadButtonChangedEvent>()
            .add_message::<bevy::input::mouse::MouseWheel>();
    });
    // The Tab observer attaches to the windows that exist at startup.
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let root = app.world().resource::<crate::bridge::JsBridge>().nodes[&ROOT_ID];
    app.world_mut().entity_mut(root).insert(TabGroup::new(0));
    let mut ops = Vec::new();
    for (id, index) in [(1, 0), (2, -1), (3, 0)] {
        ops.push(create(
            id,
            "node",
            json!({ "tabIndex": index, "onFocus": true }),
        ));
        ops.push(Op::Append {
            parent: ROOT_ID,
            child: id,
        });
    }
    tx.send(ops).unwrap();
    app.update();
    // No visibility propagation here; Tab skips hidden subtrees.
    for id in 1..=3 {
        let entity = ent(&app, id);
        app.world_mut()
            .entity_mut(entity)
            .insert(InheritedVisibility::VISIBLE);
    }

    let tab = |app: &mut App| {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut().write_message(KeyboardInput {
                key_code: KeyCode::Tab,
                logical_key: Key::Tab,
                state,
                text: None,
                repeat: false,
                window,
            });
            app.update();
        }
        focused(app)
    };
    assert_eq!(tab(&mut app), Some(ent(&app, 1)));
    assert_eq!(tab(&mut app), Some(ent(&app, 3)), "-1 is skipped");
    assert_eq!(tab(&mut app), Some(ent(&app, 1)), "and Tab wraps");
    assert!(app.world().resource::<InputFocusVisible>().0);
    assert_eq!(
        events(&mut rx),
        sent(&[(1, "focus"), (3, "focus"), (1, "focus")])
    );
}

/// Enter and Space click a focused `<button>`, once per key press: not on a
/// repeat, not on the release, and only a `<button>` (a focused
/// `<node onClick>` is not activated, as on the web).
#[test]
fn enter_and_space_click_a_focused_button() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::{ButtonState, InputPlugin};
    use bevy::input_focus::InputDispatchPlugin;
    use bevy::window::PrimaryWindow;

    let (mut app, tx, mut rx) = focus_app(|app| {
        // Bevy's own button widget handles the same keys (and stops their
        // propagation), as in a full app.
        app.add_plugins((
            InputPlugin,
            InputDispatchPlugin,
            bevy::ui_widgets::ButtonPlugin,
        ))
        // What the key dispatch reads in a full app.
        .add_message::<bevy::input::gamepad::GamepadButtonChangedEvent>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_observer(super::click_focused_button);
    });
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    tx.send(vec![
        create(1, "button", json!({ "tabIndex": 0, "onClick": true })),
        create(2, "node", json!({ "tabIndex": 0, "onClick": true })),
    ])
    .unwrap();
    app.update();
    let mut key = |app: &mut App, key_code, state, repeat| {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: Key::Enter,
            state,
            text: None,
            repeat,
            window,
        });
        app.update();
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|out| match out {
                Outbound::UiEvent { event } => Some((event.id, event.kind)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let click = [(1, "click".to_owned())];
    let (down, up) = (ButtonState::Pressed, ButtonState::Released);

    focus(&mut app, 1);
    assert_eq!(key(&mut app, KeyCode::Enter, down, false), click);
    assert_eq!(key(&mut app, KeyCode::Enter, down, true), [], "a repeat");
    assert_eq!(key(&mut app, KeyCode::Enter, up, false), [], "the release");
    assert_eq!(key(&mut app, KeyCode::Space, down, false), click);
    assert_eq!(key(&mut app, KeyCode::KeyA, down, false), []);

    focus(&mut app, 2);
    assert_eq!(key(&mut app, KeyCode::Enter, down, false), [], "a node");
}
