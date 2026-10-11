//! Element registry tests through the op-apply harness: default styles on
//! the live entity, `styleIgnored`, event subscriptions and
//! [`ElementEvents`] gating.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::{ElementEvents, EventSubscriptions};
use crate::bridge::{OutboundResource, StyleVariants};
use crate::protocol::op::Op;
use crate::protocol::outbound::Outbound;
use crate::reconcile::test_util::{create, ent, op_app, op_app_with, update};
use crate::style::props::FOCUS_POLICY;

/// Swap the harness's leaked outbound channel for one the test can read.
fn observe_outbound(app: &mut App) -> tokio::sync::mpsc::UnboundedReceiver<Outbound> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    app.insert_resource(OutboundResource(tx));
    rx
}

fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Outbound>) -> Vec<Outbound> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

/// A `<button>` blocks the pointer by default (its default style); a user
/// `focusPolicy` overrides it, and unsetting that falls back to the default,
/// not to the global `Pass`. Its hover/press base is the effective style, so
/// a restyle keeps the default too.
#[test]
fn button_default_style_falls_back_on_unset() {
    let (mut app, tx) = op_app();
    tx.send(vec![create(
        1,
        "button",
        serde_json::json!({ "hoverStyle": { "backgroundColor": "red" } }),
    )])
    .unwrap();
    app.update();
    let e = ent(&app, 1);
    assert_eq!(app.world().get::<FocusPolicy>(e), Some(&FocusPolicy::Block));
    assert_eq!(
        app.world()
            .get::<StyleVariants>(e)
            .and_then(|v| v.base.as_ref())
            .and_then(|s| s.get(&FOCUS_POLICY)),
        Some(&FocusPolicy::Block),
        "the variants' base is the effective style"
    );

    tx.send(vec![update(
        1,
        "button",
        serde_json::json!({ "style": { "focusPolicy": "pass" } }),
        &[],
        &[],
    )])
    .unwrap();
    app.update();
    assert_eq!(app.world().get::<FocusPolicy>(e), Some(&FocusPolicy::Pass));

    tx.send(vec![update(
        1,
        "button",
        serde_json::json!({}),
        &[],
        &["focusPolicy"],
    )])
    .unwrap();
    app.update();
    assert_eq!(
        app.world().get::<FocusPolicy>(e),
        Some(&FocusPolicy::Block),
        "unsetting the user value shows the default through"
    );
}

/// `backgroundImage` on an `<image>` never reaches its element-owned
/// `ImageNode` (the global writer is masked off there) and warns
/// `styleIgnored`.
#[cfg(all(feature = "devtools", debug_assertions))]
#[test]
fn background_image_on_image_warns_style_ignored() {
    use crate::background_image::RBackgroundTexture;
    let _lock = crate::diag::test_lock();
    crate::diag::arm_runtime();
    let _ = crate::diag::take_runtime_warnings();

    let (mut app, tx) = op_app();
    tx.send(vec![create(
        77,
        "image",
        serde_json::json!({
            "src": "a.png",
            "style": { "backgroundImage": { "src": { "texture": "x" } } },
        }),
    )])
    .unwrap();
    app.update();
    let e = ent(&app, 77);
    assert!(app.world().get::<RBackgroundTexture>(e).is_none());
    let mine: Vec<_> = crate::diag::take_runtime_warnings()
        .into_iter()
        .filter(|w| w.node == Some(77))
        .collect();
    assert!(
        mine.iter()
            .any(|w| w.kind == "styleIgnored" && w.value == "backgroundImage"),
        "got {mine:?}"
    );
}

/// A test element with one `unconditional` event (the canvas `resize`
/// pattern: a runtime-consumed event sent whether or not a handler exists).
static PING: super::ElementEvent<f32> = super::ElementEvent::new("ping").unconditional();
static PROBE: super::Element = super::Element {
    events: &[&PING],
    ..super::Element::new("probe")
};

/// An element event reaches JS only while the node declares its handler:
/// the op path keeps `EventSubscriptions` in step with the handler props,
/// and `ElementEvents::send` checks it. An `unconditional` event skips the
/// gate.
#[test]
fn element_events_follow_subscriptions() {
    use crate::ReactAppExt;
    use crate::elements::editable::CHANGE;
    let (mut app, tx) = op_app_with(|app| {
        app.add_react_element(&PROBE);
    });
    let mut rx = observe_outbound(&mut app);
    tx.send(vec![
        create(1, "editableText", serde_json::json!({ "value": "a" })),
        create(2, "probe", serde_json::json!({})),
    ])
    .unwrap();
    app.update();
    let (input, probe) = (ent(&app, 1), ent(&app, 2));
    assert!(app.world().get::<EventSubscriptions>(input).is_none());

    let send_change = move |events: ElementEvents| events.send(input, &CHANGE, &"b".to_owned());
    assert!(
        !app.world_mut().run_system_once(send_change).unwrap(),
        "no onChange: nothing sent"
    );
    let ping = move |events: ElementEvents| events.send(probe, &PING, &4.0);
    assert!(
        app.world_mut().run_system_once(ping).unwrap(),
        "an unconditional event needs no handler"
    );

    tx.send(vec![update(
        1,
        "editableText",
        serde_json::json!({ "onChange": true }),
        &[],
        &[],
    )])
    .unwrap();
    app.update();
    assert!(app.world().get::<EventSubscriptions>(input).is_some());
    assert!(app.world_mut().run_system_once(send_change).unwrap());

    let sent = drain(&mut rx);
    let brief: Vec<_> = sent
        .iter()
        .filter_map(|o| match o {
            Outbound::ElementEvent { id, event, payload } => {
                Some((*id, event.clone(), payload.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        brief,
        vec![
            (2, "ping".to_owned(), serde_json::json!(4.0)),
            (1, "change".to_owned(), serde_json::json!("b")),
        ]
    );

    tx.send(vec![Op::Update {
        id: 1,
        props: Box::default(),
        unset: vec!["onChange".into()],
        style_unset: vec![],
    }])
    .unwrap();
    app.update();
    assert!(app.world().get::<EventSubscriptions>(input).is_none());
    assert!(!app.world_mut().run_system_once(send_change).unwrap());
}

/// An input's act-now `value` seeds it at create: the spawn sees the
/// create's act-now attributes before they are split off.
#[test]
fn editable_value_seeds_on_create() {
    let (mut app, tx) = op_app();
    tx.send(vec![create(
        1,
        "editableText",
        serde_json::json!({ "value": "NYX" }),
    )])
    .unwrap();
    app.update();
    let input = ent(&app, 1);
    let editable = app
        .world()
        .get::<bevy::text::EditableText>(input)
        .expect("an input");
    assert_eq!(editable.value().to_string(), "NYX");
}

/// An `<editableText>` is a working text input: it carries bevy's
/// `TextInput` (its key/pointer/IME systems act on nothing else), announces
/// as a text field rather than `TextInput`'s bare default, and blocks the
/// picks beneath it even at the default pass-through `focusPolicy` — so a
/// click is one press, and one focus request, on the input.
#[test]
fn editable_is_a_focusable_text_input() {
    let (mut app, tx) = op_app();
    tx.send(vec![
        create(
            1,
            "editableText",
            serde_json::json!({ "ariaLabel": "Name" }),
        ),
        create(2, "node", serde_json::json!({})),
    ])
    .unwrap();
    app.update();
    let (input, node) = (ent(&app, 1), ent(&app, 2));
    let world = app.world();
    assert!(world.get::<bevy::ui_widgets::TextInput>(input).is_some());
    assert!(
        world
            .get::<bevy::input_focus::tab_navigation::TabIndex>(input)
            .is_some()
    );
    let a11y = world
        .get::<bevy::a11y::AccessibilityNode>(input)
        .expect("an accessibility node");
    assert_eq!(a11y.label(), Some("Name"), "our node, not the default");
    let pickable = |e| *world.get::<bevy::picking::Pickable>(e).unwrap();
    assert!(pickable(input).should_block_lower);
    assert!(
        !pickable(node).should_block_lower,
        "plain nodes pass through"
    );
}

/// End to end with bevy's own input path: a key typed while the input holds
/// the focus reaches its `EditableText`. (Bevy's key handler only acts on
/// `TextInput` entities — an `EditableText` alone would ignore it.)
#[test]
fn typed_keys_reach_a_focused_editable() {
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::{ButtonState, InputPlugin};
    use bevy::input_focus::{InputDispatchPlugin, InputFocus};
    use bevy::text::TextEdit;
    use bevy::window::{PrimaryWindow, Window};

    let (mut app, tx) = op_app_with(|app| {
        app.add_plugins((
            InputPlugin,
            InputDispatchPlugin,
            bevy::ui_widgets::TextInputPlugin,
        ));
        // What the widget's IME/autoscroll systems read in a full app.
        app.init_resource::<InputFocus>()
            .init_resource::<Time<bevy::time::Real>>()
            .init_resource::<bevy::picking::events::PointerState>()
            .init_resource::<bevy::ui::UiScale>()
            .add_message::<bevy::window::Ime>()
            .add_message::<bevy::input::gamepad::GamepadButtonChangedEvent>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .add_message::<bevy::picking::events::PointerRelease>();
    });
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    tx.send(vec![create(1, "editableText", serde_json::json!({}))])
        .unwrap();
    app.update();
    let input = ent(&app, 1);
    app.insert_resource(InputFocus::from_entity(input));
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character("a".into()),
        state: ButtonState::Pressed,
        text: Some("a".into()),
        repeat: false,
        window,
    });
    app.update();
    let editable = app
        .world()
        .get::<bevy::text::EditableText>(input)
        .expect("an input");
    assert!(
        editable
            .pending_edits
            .contains(&TextEdit::Insert("a".into()))
            || editable.value() == "a",
        "the typed key reached the input: pending {:?}, value {:?}",
        editable.pending_edits,
        editable.value().to_string()
    );
}

/// A `<button>` announces as a button labelled with the text mounted under
/// it — nested wrappers included, in tree order — and follows its children.
#[test]
fn button_is_labelled_from_its_text() {
    let (mut app, tx) = op_app_with(|app| {
        app.add_systems(
            Update,
            crate::elements::label_buttons.after(crate::reconcile::apply_js_ops),
        );
    });
    let text = |id, s: &str| Op::Create {
        id,
        kind: "text".into(),
        props: Box::default(),
        text: Some(s.into()),
    };
    tx.send(vec![
        create(1, "button", serde_json::json!({})),
        create(2, "node", serde_json::json!({})),
        text(3, "Save"),
        text(4, "draft"),
        Op::Append {
            parent: 2,
            child: 3,
        },
        Op::Append {
            parent: 1,
            child: 2,
        },
        Op::Append {
            parent: 1,
            child: 4,
        },
    ])
    .unwrap();
    app.update();
    app.update();
    let button = ent(&app, 1);
    let label = |app: &App| {
        app.world()
            .get::<bevy::a11y::AccessibilityNode>(button)
            .and_then(|n| n.label().map(str::to_owned))
    };
    assert_eq!(label(&app).as_deref(), Some("Save draft"));
    assert_eq!(
        app.world()
            .get::<bevy::a11y::AccessibilityNode>(button)
            .unwrap()
            .role(),
        accesskit::Role::Button
    );

    tx.send(vec![Op::Remove {
        parent: 1,
        child: 4,
    }])
    .unwrap();
    app.update();
    app.update();
    assert_eq!(label(&app).as_deref(), Some("Save"));
}
