use super::super::test_util::{ent, op_app, update_delta};
use super::*;
use crate::protocol::op::Op;
use bevy::ui::FocusPolicy;

/// A plain `<node onClick>` — no hover/press style, not a `<button>` — must get
/// a `ReactInteraction` so `collect_ui_events` can report its clicks. Regression:
/// `onClick` crossed the wire as a bool but nothing attached a `ReactInteraction`,
/// so such a node was silently unclickable (only a `<button>`, or a node that
/// also had a hover/press style or an `onPointer*` handler, worked).
#[test]
fn node_onclick_attaches_interaction() {
    let (mut app, ops_tx) = op_app();

    ops_tx
        .send(vec![
            // 1: a bare onClick node — the case that was broken.
            Op::Create {
                id: 1,
                kind: "node".into(),
                props: serde_json::from_value(serde_json::json!({ "onClick": true })).unwrap(),
                text: None,
            },
            // 2: a node with no interaction props at all — must stay inert.
            Op::Create {
                id: 2,
                kind: "node".into(),
                props: Box::default(),
                text: None,
            },
        ])
        .unwrap();
    app.update();

    let nodes = &app.world().resource::<JsBridge>().nodes;
    let (clickable, inert) = (nodes[&1], nodes[&2]);
    assert!(
        app.world()
            .entity(clickable)
            .get::<ReactInteraction>()
            .is_some(),
        "`onClick` alone must make a <node> clickable"
    );
    assert!(
        app.world()
            .entity(inert)
            .get::<ReactInteraction>()
            .is_none(),
        "a node with no handlers/hover/press must not gain a ReactInteraction"
    );
}

/// `ClickOwner` tracks handler presence, independent of `ReactInteraction`: a
/// hover-styled node without handlers is interactive for styling but not
/// a click owner; toggling `onClick` off drops ownership while the
/// hover `ReactInteraction` survives.
#[test]
fn click_owner_tracks_handlers_not_styling() {
    use crate::bridge::ClickOwner;
    let (mut app, ops_tx) = op_app();
    ops_tx
        .send(vec![
            // 1: hover-styled only — interactive, but NOT a click owner.
            Op::Create {
                id: 1,
                kind: "node".into(),
                props: serde_json::from_value(
                    serde_json::json!({ "hoverStyle": { "opacity": 0.5 } }),
                )
                .unwrap(),
                text: None,
            },
            // 2: onClick + hover style — both.
            Op::Create {
                id: 2,
                kind: "node".into(),
                props: serde_json::from_value(
                    serde_json::json!({ "onClick": true, "hoverStyle": { "opacity": 0.5 } }),
                )
                .unwrap(),
                text: None,
            },
        ])
        .unwrap();
    app.update();

    let nodes = app.world().resource::<JsBridge>().nodes.clone();
    let (styled, clickable) = (nodes[&1], nodes[&2]);
    assert!(
        app.world()
            .entity(styled)
            .get::<ReactInteraction>()
            .is_some(),
        "hover styling makes the node interactive"
    );
    assert!(
        app.world().entity(styled).get::<ClickOwner>().is_none(),
        "…but never a click owner"
    );
    assert!(app.world().entity(clickable).get::<ClickOwner>().is_some());

    // Toggling the handler off drops ownership but keeps the hover
    // `ReactInteraction` (the style variant still needs it).
    ops_tx
        .send(vec![update_delta(2, Props::default(), &["onClick"], &[])])
        .unwrap();
    app.update();
    let entity = app.world().entity(clickable);
    assert!(
        entity.get::<ClickOwner>().is_none(),
        "unsetting the last handler drops click ownership"
    );
    assert!(
        entity.get::<ReactInteraction>().is_some(),
        "the hover-style ReactInteraction survives"
    );
}

/// A `<text>` root is fully interactive: create stamps the hover/press
/// variants + handler markers (`stamp_common`), and a delta refreshes
/// them — same contract as a `<node>`.
#[test]
fn text_root_stamps_interactivity() {
    use crate::bridge::ClickOwner;
    let (mut app, ops_tx) = op_app();
    ops_tx
        .send(vec![Op::Create {
            id: 1,
            kind: "text".into(),
            props: serde_json::from_value(serde_json::json!({
                "onClick": true,
                "hoverStyle": { "color": "blue" },
            }))
            .unwrap(),
            text: Some("label".into()),
        }])
        .unwrap();
    app.update();

    let e = app.world().resource::<JsBridge>().nodes[&1];
    let entity = app.world().entity(e);
    assert!(
        entity.get::<StyleVariants>().is_some(),
        "a <text> hoverStyle stamps StyleVariants"
    );
    assert!(entity.get::<ReactInteraction>().is_some());
    assert!(
        entity.get::<ClickOwner>().is_some(),
        "onClick makes the <text> a click owner"
    );

    // A delta adding a pointer handler lands too (the text update branch
    // mirrors the general arm).
    ops_tx
        .send(vec![update_delta(
            1,
            serde_json::from_value(serde_json::json!({ "onPointerDown": true })).unwrap(),
            &[],
            &[],
        )])
        .unwrap();
    app.update();
    assert!(
        app.world()
            .entity(e)
            .get::<PointerHandlers>()
            .is_some_and(|h| h.down),
        "a handler delta must reach a <text> root"
    );
}

/// A node with `onPointerEnter`/`onPointerLeave`/`onPointerHover` gets an
/// `ReactInteraction` + a [`HoverState`], and the reconciler stamps the handler
/// flags.
#[test]
fn pointer_enter_leave_stamps_hover_state() {
    let (mut app, ops_tx) = op_app();
    ops_tx
        .send(vec![Op::Create {
            id: 1,
            kind: "node".into(),
            props: serde_json::from_value(serde_json::json!({
                "onPointerEnter": true,
                "onPointerLeave": true,
                "onPointerHover": true,
            }))
            .unwrap(),
            text: None,
        }])
        .unwrap();
    app.update();

    let e = app.world().resource::<JsBridge>().nodes[&1];
    let entity = app.world().entity(e);
    assert!(
        entity.get::<ReactInteraction>().is_some(),
        "hover handlers must make the node interactive"
    );
    assert!(
        entity.get::<HoverState>().is_some(),
        "hover handlers must stamp a HoverState"
    );
    let handlers = entity.get::<PointerHandlers>().expect("PointerHandlers");
    assert!(handlers.enter && handlers.leave && handlers.hover);
}

/// `imageRendering` stamps `ImageRenderingMode` for an explicit mode only:
/// `"auto"` is passive (no component, like absent), a delta flips it, and
/// `styleUnset` removes it.
#[test]
fn image_rendering_mode_is_stamped_for_explicit_modes_only() {
    use crate::image_rendering::{ImageRendering, ImageRenderingMode};
    let (mut app, ops_tx) = op_app();
    let styled = |mode: &str| -> Box<Props> {
        Box::new(
            serde_json::from_value(serde_json::json!({ "style": { "imageRendering": mode } }))
                .unwrap(),
        )
    };
    let create = |id: NodeId, props: Box<Props>| Op::Create {
        id,
        kind: "image".into(),
        props,
        text: None,
    };
    ops_tx
        .send(vec![
            create(1, styled("trilinear")),
            create(2, styled("auto")),
            create(3, Box::default()),
        ])
        .unwrap();
    app.update();
    let mode = |app: &App, id: NodeId| -> Option<ImageRendering> {
        app.world()
            .entity(ent(app, id))
            .get::<ImageRenderingMode>()
            .map(|m| m.0)
    };
    assert_eq!(mode(&app, 1), Some(ImageRendering::Trilinear));
    assert_eq!(mode(&app, 2), None, "auto is passive");
    assert_eq!(mode(&app, 3), None);

    ops_tx
        .send(vec![
            update_delta(1, Props::default(), &[], &["imageRendering"]),
            update_delta(3, *styled("nearest"), &[], &[]),
        ])
        .unwrap();
    app.update();
    assert_eq!(mode(&app, 1), None, "styleUnset removes the mode");
    assert_eq!(mode(&app, 3), Some(ImageRendering::Nearest));
}

/// `FocusPolicy` defaults differ by element kind: a `<button>` captures the
/// pointer (`Block`, mirroring bevy_ui's native `Button`), while a `<node>`
/// passes it through (`Pass`), so a container/label never swallows clicks meant
/// for what's behind it. An explicit `focusPolicy` prop overrides either, and
/// re-rendering a button keeps its `Block` (the per-commit `apply_style_masked` resets
/// it to `Pass` first).
#[test]
fn focus_policy_defaults_block_button_pass_node() {
    let (mut app, ops_tx) = op_app();

    let node_props = |json: serde_json::Value| -> Props { serde_json::from_value(json).unwrap() };
    ops_tx
        .send(vec![
            // 1: bare button → Block default.
            Op::Create {
                id: 1,
                kind: "button".into(),
                props: Box::default(),
                text: None,
            },
            // 2: bare node → Pass default.
            Op::Create {
                id: 2,
                kind: "node".into(),
                props: Box::default(),
                text: None,
            },
            // 3: button with explicit focusPolicy "pass" → overrides the default.
            Op::Create {
                id: 3,
                kind: "button".into(),
                props: Box::new(node_props(
                    serde_json::json!({ "style": { "focusPolicy": "pass" } }),
                )),
                text: None,
            },
        ])
        .unwrap();
    app.update();

    let fp = |app: &App, id: u32| -> Option<FocusPolicy> {
        let e = app.world().resource::<JsBridge>().nodes[&id];
        app.world().entity(e).get::<FocusPolicy>().copied()
    };
    // The picking mirror: `Pickable.should_block_lower` must track the policy,
    // because the picking backend (which clicks and all `<surface>` interaction
    // ride) ignores `FocusPolicy` and blocks when `Pickable` is absent.
    let blocks = |app: &App, id: u32| -> Option<bool> {
        let e = app.world().resource::<JsBridge>().nodes[&id];
        app.world()
            .entity(e)
            .get::<bevy::picking::Pickable>()
            .map(|p| p.should_block_lower)
    };
    assert_eq!(
        fp(&app, 1),
        Some(FocusPolicy::Block),
        "button defaults to Block"
    );
    assert_eq!(blocks(&app, 1), Some(true), "button blocks picking too");
    assert_eq!(
        fp(&app, 2),
        Some(FocusPolicy::Pass),
        "node defaults to Pass"
    );
    assert_eq!(blocks(&app, 2), Some(false), "node passes picking too");
    assert_eq!(
        fp(&app, 3),
        Some(FocusPolicy::Pass),
        "explicit focusPolicy overrides the button default"
    );
    assert_eq!(
        blocks(&app, 3),
        Some(false),
        "explicit pass unblocks picking on a button"
    );

    // A delta that re-runs the focus-policy writer (unsetting the —
    // already absent — `focusPolicy` property) must keep the bare button
    // at its kind default, `Block`. (A delta touching nothing wouldn't run
    // the writer at all.)
    ops_tx
        .send(vec![update_delta(
            1,
            Props::default(),
            &[],
            &["focusPolicy"],
        )])
        .unwrap();
    app.update();
    assert_eq!(
        fp(&app, 1),
        Some(FocusPolicy::Block),
        "a re-rendered button keeps its Block default"
    );
    assert_eq!(
        blocks(&app, 1),
        Some(true),
        "a re-rendered button keeps blocking picking"
    );
}

/// A node created with a controlled `scrollTop` gets that `ScrollPosition`; an
/// `onScroll` node gets a `ScrollListener` and is seeded in the dedup map (at
/// `ZERO` when uncontrolled) so its mount-frame change doesn't echo back.
#[test]
fn controlled_scroll_create_sets_position_and_listener() {
    let (mut app, ops_tx) = op_app();
    ops_tx
        .send(vec![
            // controlled offset + an onScroll handler.
            Op::Create {
                id: 1,
                kind: "node".into(),
                props: serde_json::from_value(serde_json::json!({
                    "scrollTop": 50.0, "onScroll": true,
                    "style": { "overflowY": "scroll" }
                }))
                .unwrap(),
                text: None,
            },
            // listener only (read-only scroll): seeded at ZERO.
            Op::Create {
                id: 2,
                kind: "node".into(),
                props: serde_json::from_value(serde_json::json!({ "onScroll": true })).unwrap(),
                text: None,
            },
            // controlled only, no handler → no marker.
            Op::Create {
                id: 3,
                kind: "node".into(),
                props: serde_json::from_value(serde_json::json!({ "scrollTop": 30.0 })).unwrap(),
                text: None,
            },
        ])
        .unwrap();
    app.update();

    let nodes = app.world().resource::<JsBridge>().nodes.clone();
    let (e1, e2, e3) = (nodes[&1], nodes[&2], nodes[&3]);

    assert_eq!(
        app.world().entity(e1).get::<ScrollPosition>().unwrap().0,
        Vec2::new(0.0, 50.0)
    );
    assert!(app.world().entity(e1).get::<ScrollListener>().is_some());
    assert!(app.world().entity(e2).get::<ScrollListener>().is_some());
    assert!(
        app.world().entity(e3).get::<ScrollListener>().is_none(),
        "a controlled node with no onScroll must not be marked"
    );

    let bridge = app.world().resource::<JsBridge>();
    assert_eq!(bridge.scroll_positions.get(&1), Some(&Vec2::new(0.0, 50.0)));
    assert_eq!(bridge.scroll_positions.get(&2), Some(&Vec2::ZERO));
    assert_eq!(bridge.scroll_positions.get(&3), Some(&Vec2::new(0.0, 30.0)));
}

/// A controlled `scrollTop` past the scrollable range clamps the written
/// `ScrollPosition` to the max, while recording the *requested* value so the
/// read-back can correct React's controlled state down to the real max.
#[test]
fn controlled_scroll_update_clamps_to_range() {
    let (mut app, ops_tx) = op_app();
    ops_tx
        .send(vec![Op::Create {
            id: 1,
            kind: "node".into(),
            props: serde_json::from_value(serde_json::json!({
                "onScroll": true, "style": { "overflowY": "scroll" }
            }))
            .unwrap(),
            text: None,
        }])
        .unwrap();
    app.update();

    let e1 = app.world().resource::<JsBridge>().nodes[&1];
    // A laid-out size with real range: content 300, view 100 → max scroll 200.
    app.world_mut().entity_mut(e1).insert(ComputedNode {
        size: Vec2::new(200.0, 100.0),
        content_size: Vec2::new(200.0, 300.0),
        inverse_scale_factor: 1.0,
        ..default()
    });

    ops_tx
        .send(vec![update_delta(
            1,
            serde_json::from_value(serde_json::json!({
                "onScroll": true, "scrollTop": 10000.0,
                "style": { "overflowY": "scroll" }
            }))
            .unwrap(),
            &[],
            &[],
        )])
        .unwrap();
    app.update();

    assert_eq!(
        app.world().entity(e1).get::<ScrollPosition>().unwrap().0,
        Vec2::new(0.0, 200.0),
        "the written offset is clamped to the scrollable range"
    );
    assert_eq!(
        app.world().resource::<JsBridge>().scroll_positions.get(&1),
        Some(&Vec2::new(0.0, 10000.0)),
        "the requested (pre-clamp) value is recorded so the read-back can correct React"
    );
}

/// With a `transition: { scroll }`, a controlled `scrollTop` change sets the eased
/// `ScrollTransitionState` target instead of snapping `ScrollPosition` — the drive
/// system (not exercised here) moves the offset toward it.
#[test]
fn controlled_scroll_with_transition_sets_target_not_position() {
    let (mut app, ops_tx) = op_app();
    let style = serde_json::json!({
        "overflowY": "scroll", "transition": { "scroll": { "duration": 300 } }
    });
    ops_tx
        .send(vec![Op::Create {
            id: 1,
            kind: "node".into(),
            props: serde_json::from_value(serde_json::json!({ "style": style })).unwrap(),
            text: None,
        }])
        .unwrap();
    app.update();

    let e1 = app.world().resource::<JsBridge>().nodes[&1];
    // A real scroll range so the target isn't clamped away (content 300, view 100).
    app.world_mut().entity_mut(e1).insert(ComputedNode {
        size: Vec2::new(200.0, 100.0),
        content_size: Vec2::new(200.0, 300.0),
        inverse_scale_factor: 1.0,
        ..default()
    });

    ops_tx
        .send(vec![update_delta(
            1,
            serde_json::from_value(serde_json::json!({ "scrollTop": 80.0, "style": style }))
                .unwrap(),
            &[],
            &[],
        )])
        .unwrap();
    app.update();

    assert_eq!(
        app.world().entity(e1).get::<ScrollPosition>().unwrap().0,
        Vec2::ZERO,
        "a controlled change with a scroll transition must not snap the offset"
    );
    assert_eq!(
        app.world()
            .entity(e1)
            .get::<ScrollTransitionState>()
            .unwrap()
            .target,
        Vec2::new(0.0, 80.0),
        "it sets the eased target instead"
    );
}
