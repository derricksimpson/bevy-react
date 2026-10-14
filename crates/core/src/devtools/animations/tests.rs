//! The Animations tab's stream, seeks and clock.

use super::clock::DevtoolsAnimationsClockMessage;
use super::describe::{describe, span};
use super::*;
use crate::animations::{
    AnimatableProperty, AnimatedBindings, Binding, Driver, Easing, Run, Runner,
};
use crate::devtools::DevtoolsConfig;
use crate::devtools::test_util::{drain_events, test_app};
use crate::ext::AnimationRuns;
use crate::message::ReactAppExt;
use crate::protocol::outbound::Outbound;
use crate::reconcile::OpApplyStats;
use bevy::time::TimeUpdateStrategy;
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;

fn payloads(rx: &mut UnboundedReceiver<Outbound>) -> Vec<serde_json::Value> {
    drain_events(rx)
        .into_iter()
        .filter(|(name, _)| name == "devtools.animations")
        .map(|(_, v)| v)
        .collect()
}

fn bound_to(id: SharedId) -> AnimatedNode {
    AnimatedNode(AnimatedBindings(
        [(AnimatableProperty::Opacity, Binding::Shared { id })].into(),
    ))
}

/// The stream is gated on the open tab and samples every `SAMPLE`: it
/// lists running drivers first (each stage with its live phase, the
/// meter's range), then idle values app nodes read — never an idle
/// unbound value, nor one only the panel's own nodes read — stays silent
/// while nothing changes, and resends in full when the tab is re-shown.
#[test]
fn animations_stream_lists_running_and_bound_values() {
    let (mut app, mut rx) = test_app(DevtoolsConfig {
        settings_path: None,
        ..default()
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        50,
    )));
    // The panel's own meter binds value 1: under the panel root, so it
    // must not list it.
    let panel = app.world_mut().spawn(ReactNode(90)).id();
    app.world_mut()
        .spawn((ReactNode(91), bound_to(1), ChildOf(panel)));
    let mut bridge = JsBridge::new(
        crossbeam_channel::unbounded().1,
        tokio::sync::mpsc::unbounded_channel().0,
        Entity::PLACEHOLDER,
    );
    bridge.nodes.insert(90, panel);
    app.insert_resource(bridge);
    app.world_mut().spawn((ReactNode(10), bound_to(2)));
    app.world_mut().spawn((ReactNode(11), bound_to(2)));
    {
        let mut values = app.world_mut().resource_mut::<SharedValues>();
        values.declare(1, 0.0);
        values.declare(2, 0.5);
        values.declare(4, 0.0);
        let timing = Driver::Timing {
            to: 100.0,
            duration: 1.0,
            easing: Easing::EaseInOut,
        };
        values.animate(
            3,
            &Driver::Repeat {
                animation: Box::new(timing),
                count: -1,
                reverse: true,
            },
            None,
        );
    }

    app.update();
    assert!(payloads(&mut rx).is_empty(), "closed tab: no stream");

    {
        let mut state = app.world_mut().resource_mut::<DevtoolsState>();
        state.open = true;
        state.animations_tab_open = true;
        state.panel_root = Some(90);
    }
    app.update();
    let row3 = |value: f64, timing: &str, at: f64| {
        json!({
            "id": 3,
            "value": value,
            "driver": ["repeat 1 of inf, reverse", timing],
            "range": [0.0, 100.0],
            // A loop's scrubber spans its current cycle.
            "time": [at, 0.0, 1.0],
            "nodes": [],
        })
    };
    let row2 = json!({
        "id": 2, "value": 0.5, "driver": [], "range": null, "time": null, "nodes": [10, 11],
    });
    let table = |values: serde_json::Value| {
        json!({
            "values": values,
            "total": 2,
            "transitions": [],
            "transitions_total": 0,
            "clock": { "paused": false, "speed": 1.0 },
        })
    };
    assert_eq!(
        payloads(&mut rx),
        [table(json!([
            row3(0.0, "timing 0 -> 100, 1000ms easeInOut, 0%", 0.0),
            row2
        ]))],
        "opening the tab sends the table at once"
    );

    // A quarter through the timing; the next frame is inside `SAMPLE`.
    app.world_mut().resource_mut::<SharedValues>().tick(0.25);
    app.update();
    assert!(payloads(&mut rx).is_empty(), "sampled every SAMPLE");
    app.update();
    assert_eq!(
        payloads(&mut rx),
        [table(json!([
            row3(6.25, "timing 0 -> 100, 1000ms easeInOut, 25%", 0.25),
            row2
        ]))],
        "the live phase and reading"
    );

    app.update();
    app.update();
    assert!(payloads(&mut rx).is_empty(), "an unchanged table is silent");

    app.world_mut()
        .resource_mut::<DevtoolsState>()
        .animations_tab_open = false;
    app.update();
    assert!(payloads(&mut rx).is_empty(), "hidden tab: silence");
    app.world_mut()
        .resource_mut::<DevtoolsState>()
        .animations_tab_open = true;
    app.update();
    assert_eq!(payloads(&mut rx).len(), 1, "a re-shown tab gets it all");
}

/// Every stage of a composite driver gets a line, with its live phase,
/// and the range spans every value the whole tree aims for.
#[test]
fn describes_composite_drivers() {
    let timing = |to| Driver::Timing {
        to,
        duration: 0.5,
        easing: Easing::Linear,
    };
    let driver = Driver::Sequence {
        steps: vec![
            timing(110.0),
            Driver::Delay {
                delay: 0.25,
                animation: Box::new(timing(-110.0)),
            },
            Driver::Spring {
                to: 0.0,
                stiffness: 100.0,
                damping: 10.0,
                mass: 1.0,
            },
        ],
    };
    let mut runner = crate::animations::build_runner(&driver, 20.0);
    let lines = |runner: &Runner| {
        let mut out = Vec::new();
        describe(runner, &mut out);
        out
    };
    assert_eq!(
        lines(&runner),
        ["sequence 1 of 3", "timing 20 -> 110, 500ms linear, 0%"]
    );
    assert_eq!(span(&runner, 20.0), Some((-110.0, 110.0)));
    runner.step(0.5);
    runner.step(0.125);
    assert_eq!(lines(&runner), ["sequence 2 of 3", "delay, 125ms left"]);
    assert_eq!(
        span(&runner, 20.0),
        Some((-110.0, 110.0)),
        "stable while it runs"
    );
    assert_eq!(
        span(&crate::animations::build_runner(&timing(0.0), 0.0), 0.0),
        None
    );
}

/// A third-party animation state, listed through the same seam the core's
/// transitions use.
#[derive(Component)]
struct Spin(Option<Run>, Option<Run>);

impl AnimationRuns for Spin {
    fn visit_runs(&mut self, f: &mut dyn FnMut(&'static str, &mut Run)) {
        for run in [&mut self.0, &mut self.1].into_iter().flatten() {
            f("spin", run);
        }
    }
}

fn timing_run(duration: f32) -> Run {
    Run::new(
        Driver::Timing {
            to: 1.0,
            duration,
            easing: Easing::EaseOut,
        },
        0.0,
    )
}

/// An app with the tab shown, a bridge, and node 5 spinning: two runs
/// under one label (the line takes the first one's timeline).
fn spinning_app() -> (App, UnboundedReceiver<Outbound>, Entity) {
    let (mut app, rx) = test_app(DevtoolsConfig {
        settings_path: None,
        ..default()
    });
    app.add_react_animation_runs::<Spin>();
    let spin = app
        .world_mut()
        .spawn((
            ReactNode(5),
            Spin(Some(timing_run(0.5)), Some(timing_run(2.0))),
        ))
        .id();
    let mut bridge = JsBridge::new(
        crossbeam_channel::unbounded().1,
        tokio::sync::mpsc::unbounded_channel().0,
        Entity::PLACEHOLDER,
    );
    bridge.nodes.insert(5, spin);
    app.insert_resource(bridge);
    let mut state = app.world_mut().resource_mut::<DevtoolsState>();
    state.open = true;
    state.animations_tab_open = true;
    (app, rx, spin)
}

/// Runs a registered component reports become one row per node, one line
/// per label; a seek moves every run under the label.
#[test]
fn transition_rows_list_and_seek_runs() {
    let (mut app, mut rx, spin) = spinning_app();
    app.update();
    let sent = payloads(&mut rx);
    assert_eq!(
        sent[0]["transitions"],
        json!([{
            "node": 5,
            "lines": [{ "label": "spin", "spec": "500ms easeOut", "time": [0.0, 0.0, 0.5] }],
        }])
    );
    assert_eq!(sent[0]["transitions_total"], 1);

    app.world_mut().trigger(DevtoolsSeekRunMessage {
        node: 5,
        label: "spin".into(),
        t: 0.25,
    });
    app.update();
    let mut spinning = app.world_mut().get_mut::<Spin>(spin).unwrap();
    let (a, b) = (
        spinning.0.as_mut().unwrap().timeline(),
        spinning.1.as_mut().unwrap().timeline(),
    );
    assert_eq!((a.0, b.0), (0.25, 0.25), "both runs under the label moved");
    assert_eq!(
        spinning.0.as_mut().unwrap().step(0.0),
        (1.0 - 0.5f32.powi(3), false),
        "held at the seek's reading"
    );
}

/// A seek on a shared value moves its driver; an idle value ignores it.
#[test]
fn seek_value_moves_a_running_driver() {
    let (mut app, _rx) = test_app(DevtoolsConfig {
        settings_path: None,
        ..default()
    });
    {
        let mut values = app.world_mut().resource_mut::<SharedValues>();
        values.declare(1, 3.0);
        values.animate(
            2,
            &Driver::Timing {
                to: 100.0,
                duration: 1.0,
                easing: Easing::Linear,
            },
            None,
        );
    }
    app.world_mut()
        .trigger(DevtoolsSeekValueMessage { id: 2, t: 0.75 });
    app.world_mut()
        .trigger(DevtoolsSeekValueMessage { id: 1, t: 0.75 });
    let values = app.world().resource::<SharedValues>();
    assert_eq!(values.get(2), Some(75.0));
    assert_eq!(values.get(1), Some(3.0));
}

/// The controls drive `Time<Virtual>`; the payload reports it; the tab
/// hands the clock back as it found it when it goes away — unmounted, or
/// dropped by a JS runtime reset.
#[test]
fn the_tab_borrows_the_app_clock() {
    let (mut app, mut rx, _) = spinning_app();
    // Every frame samples.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(SAMPLE));
    app.update();
    payloads(&mut rx);
    app.world_mut().trigger(DevtoolsAnimationsClockMessage {
        paused: true,
        speed: 0.25,
    });
    app.update();
    let time = app.world().resource::<Time<Virtual>>();
    assert!(time.is_paused());
    assert_eq!(time.relative_speed(), 0.25);
    app.update();
    app.update();
    let sent = payloads(&mut rx);
    assert_eq!(
        sent.last().unwrap()["clock"],
        json!({ "paused": true, "speed": 0.25 })
    );

    app.world_mut()
        .resource_mut::<DevtoolsState>()
        .animations_tab_open = false;
    app.update();
    let time = app.world().resource::<Time<Virtual>>();
    assert!(!time.is_paused(), "unmounting the tab resumes");
    assert_eq!(time.relative_speed(), 1.0, "and resets the speed");

    // An app that paused its own clock gets it back paused, here after a
    // runtime reset with the tab still shown.
    app.world_mut()
        .resource_mut::<DevtoolsState>()
        .animations_tab_open = true;
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.world_mut().trigger(DevtoolsAnimationsClockMessage {
        paused: false,
        speed: 2.0,
    });
    app.update();
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    app.world_mut().resource_mut::<OpApplyStats>().reset_count += 1;
    app.update();
    let time = app.world().resource::<Time<Virtual>>();
    assert!(time.is_paused(), "the app's own pause is restored");
    assert_eq!(time.relative_speed(), 1.0);
}

/// The pause key is the pause chip, but only while the tab is shown; the
/// stream reports the clock the frame it changes, not at the next sample.
#[test]
fn the_pause_key_pauses_on_the_tab_only() {
    let (mut app, mut rx, _) = spinning_app();
    app.update();
    payloads(&mut rx);
    let tap = |app: &mut App| {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F11);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(KeyCode::F11);
        keys.clear();
    };
    let clocks = |rx: &mut UnboundedReceiver<Outbound>| {
        payloads(rx)
            .into_iter()
            .map(|p| p["clock"]["paused"].as_bool().unwrap())
            .collect::<Vec<_>>()
    };

    tap(&mut app);
    app.update();
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    assert_eq!(clocks(&mut rx), [true], "reported at once");
    tap(&mut app);
    app.update();
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    assert_eq!(clocks(&mut rx), [false]);

    app.world_mut()
        .resource_mut::<DevtoolsState>()
        .animations_tab_open = false;
    tap(&mut app);
    assert!(
        !app.world().resource::<Time<Virtual>>().is_paused(),
        "off the tab the key does nothing"
    );
}

