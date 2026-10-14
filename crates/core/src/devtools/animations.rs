//! The Animations tab: the shared-value table — every running driver, plus
//! every value an app node is bound to — streamed to the panel with each
//! value's live reading and driver state.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;

use crate::animations::{AnimatedNode, Driver, Easing, Runner, SharedId, SharedValues};
use crate::bridge::{JsBridge, ReactNode};
use crate::event::ReactEvents;
use crate::protocol::NodeId;
use crate::reconcile::climb;
use crate::{react_event, react_message};

use super::DevtoolsState;
use super::layers::round3;

/// How often the stream samples the table. The readouts are numbers to read,
/// not to watch move — the panel's meters are bound to the shared values
/// themselves, so the engine moves those every frame with no stream at all.
const SAMPLE: Duration = Duration::from_millis(100);

/// ponytail: rows past the cap are counted, not listed; window the list if
/// an app ever needs thousands inspected at once.
const MAX_ROWS: usize = 200;

/// Bevy → JS: the listed shared values, running drivers first, then by id.
/// Streamed by [`emit_animations`] only while the Animations tab is shown,
/// sampled every [`SAMPLE`] and diffed against the last payload, so an idle
/// table sends nothing.
#[react_event(name = "devtools.animations")]
#[derive(Clone, PartialEq)]
pub(super) struct DevtoolsAnimations {
    values: Vec<DevtoolsAnimationRow>,
    /// How many values qualified before the [`MAX_ROWS`] cap.
    total: u32,
}

/// One shared value in a `devtools.animations` payload.
#[derive(serde::Serialize, ts_rs::TS, Debug, Clone, PartialEq)]
struct DevtoolsAnimationRow {
    id: SharedId,
    /// The live reading, rounded to 3 decimals.
    value: f64,
    /// One line per driver stage, outermost first, with its live phase
    /// (see [`describe`]); empty when no driver runs.
    driver: Vec<String>,
    /// What the running driver sweeps — where it started and every value it
    /// aims for — when that is more than a point; the panel's meter maps it.
    range: Option<(f64, f64)>,
    /// The app nodes bound to this value (the panel's own never count).
    nodes: Vec<NodeId>,
}

/// JS → Bevy: the panel's Animations tab was shown/hidden. Gates
/// [`emit_animations`].
#[react_message(name = "devtools.animationsOpen")]
pub(super) struct DevtoolsAnimationsOpenMessage {
    pub(super) on: bool,
}

pub(super) fn on_animations_open_message(
    msg: On<DevtoolsAnimationsOpenMessage>,
    mut state: ResMut<DevtoolsState>,
) {
    state.animations_tab_open = msg.event().on;
}

/// Push `devtools.animations` while the panel is open on the Animations tab.
/// Lists every value with a running driver and every value an app node reads
/// (an idle, unbound value is noise). Nodes under the panel's own `<root>`
/// are skipped: its meters bind the values it lists, which would keep a
/// finished value listed forever.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_animations(
    state: Res<DevtoolsState>,
    values: Res<SharedValues>,
    time: Res<Time<Real>>,
    bridge: Option<Res<JsBridge>>,
    animated: Query<(Entity, &ReactNode, &AnimatedNode)>,
    child_of: Query<&ChildOf>,
    events: ReactEvents,
    mut stream: Local<(Option<DevtoolsAnimations>, Duration)>,
) {
    let (last, since) = &mut *stream;
    if !(state.open && state.animations_tab_open) {
        // A re-shown tab gets a fresh full payload.
        *last = None;
        return;
    }
    *since += time.delta();
    if last.is_some() && *since < SAMPLE {
        return;
    }
    *since = Duration::ZERO;

    let panel = state
        .panel_root
        .and_then(|id| bridge.as_ref().and_then(|b| b.nodes.get(&id).copied()));
    let mut bound: HashMap<SharedId, Vec<NodeId>> = HashMap::new();
    for (entity, node, anim) in &animated {
        if panel.is_some_and(|p| climb(entity, &child_of, |e| e == p).is_some()) {
            continue;
        }
        for (_, binding) in anim.0.iter() {
            bound.entry(binding.id()).or_default().push(node.0);
        }
    }
    let mut listed: Vec<_> = values
        .entries()
        .filter(|(id, _, driver)| driver.is_some() || bound.contains_key(id))
        .collect();
    listed.sort_by_key(|&(id, _, driver)| (driver.is_none(), id));
    let payload = DevtoolsAnimations {
        total: listed.len() as u32,
        values: listed
            .into_iter()
            .take(MAX_ROWS)
            .map(|(id, value, driver)| {
                let mut nodes = bound.remove(&id).unwrap_or_default();
                nodes.sort_unstable();
                nodes.dedup();
                let mut lines = Vec::new();
                if let Some((runner, _)) = driver {
                    describe(runner, &mut lines);
                }
                DevtoolsAnimationRow {
                    id,
                    value: round3(value),
                    driver: lines,
                    range: driver.and_then(|(runner, from)| span(runner, from)),
                    nodes,
                }
            })
            .collect(),
    };
    if last.as_ref() != Some(&payload) {
        events.send(&payload);
        *last = Some(payload);
    }
}

/// One display line per driver stage, outermost first, with its live phase.
/// Opaque strings the panel shows verbatim (no JS-side table to keep in
/// sync), ASCII only — the default font may lack arrow glyphs.
fn describe(runner: &Runner, out: &mut Vec<String>) {
    match runner {
        Runner::Done(v) => out.push(format!("done at {}", num(*v))),
        Runner::Timing {
            from,
            to,
            duration,
            easing,
            elapsed,
        } => {
            let progress = if *duration > 0.0 {
                (elapsed / duration).clamp(0.0, 1.0)
            } else {
                1.0
            };
            out.push(format!(
                "timing {} -> {}, {}ms {}, {}%",
                num(*from),
                num(*to),
                ms(*duration),
                easing_name(*easing),
                // Floored: a run a hair from its end must not read 100%.
                (progress * 100.0).floor()
            ));
        }
        Runner::Spring {
            to,
            stiffness,
            damping,
            mass,
            vel,
            ..
        } => out.push(format!(
            "spring -> {}, stiffness {} damping {} mass {}, velocity {}",
            num(*to),
            num(*stiffness),
            num(*damping),
            num(*mass),
            num(*vel)
        )),
        Runner::Repeat {
            remaining,
            reverse,
            iteration,
            child,
            ..
        } => {
            // `remaining` counts runs left including this one; negative = forever.
            let runs = if *remaining < 0 {
                "inf".to_string()
            } else {
                (iteration + *remaining as u32).to_string()
            };
            let reverse = if *reverse { ", reverse" } else { "" };
            out.push(format!("repeat {} of {runs}{reverse}", iteration + 1));
            describe(child, out);
        }
        Runner::Sequence {
            steps,
            index,
            child,
        } => {
            out.push(format!("sequence {} of {}", index + 1, steps.len()));
            describe(child, out);
        }
        Runner::Delay {
            remaining, child, ..
        } => match child {
            Some(child) => describe(child, out),
            None => out.push(format!("delay, {}ms left", ms(*remaining))),
        },
    }
}

/// The meter's extent: the reading the driver started from plus every value
/// it aims for. `None` for a single point — nothing to sweep.
fn span(runner: &Runner, from: f32) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (from, from);
    runner_targets(runner, &mut |v| {
        lo = lo.min(v);
        hi = hi.max(v);
    });
    let (lo, hi) = (round3(lo), round3(hi));
    (hi > lo).then_some((lo, hi))
}

/// Every value a runner aims for. Composite runners keep their full driver
/// templates (a sequence holds every step), so this is stable while it runs.
fn runner_targets(runner: &Runner, f: &mut impl FnMut(f32)) {
    match runner {
        Runner::Done(v) => f(*v),
        Runner::Timing { to, .. } | Runner::Spring { to, .. } => f(*to),
        Runner::Repeat { template, a, b, .. } => {
            f(*a);
            f(*b);
            driver_targets(template, f);
        }
        Runner::Sequence { steps, .. } => {
            for step in steps {
                driver_targets(step, f);
            }
        }
        Runner::Delay { animation, .. } => driver_targets(animation, f),
    }
}

fn driver_targets(driver: &Driver, f: &mut impl FnMut(f32)) {
    match driver {
        Driver::Timing { to, .. } | Driver::Spring { to, .. } => f(*to),
        Driver::Repeat { animation, .. } | Driver::Delay { animation, .. } => {
            driver_targets(animation, f)
        }
        Driver::Sequence { steps } => {
            for step in steps {
                driver_targets(step, f);
            }
        }
    }
}

/// A display number: 3 decimals, never `-0`.
fn num(v: f32) -> String {
    (round3(v) + 0.0).to_string()
}

fn ms(secs: f32) -> String {
    ((secs * 1000.0).round() + 0.0).to_string()
}

/// The wire name (`withTiming`'s `easing`).
fn easing_name(easing: Easing) -> &'static str {
    match easing {
        Easing::Linear => "linear",
        Easing::EaseIn => "easeIn",
        Easing::EaseOut => "easeOut",
        Easing::EaseInOut => "easeInOut",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::{AnimatableProperty, AnimatedBindings, Binding};
    use crate::devtools::DevtoolsConfig;
    use crate::devtools::test_util::{drain_events, test_app};
    use crate::protocol::outbound::Outbound;
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
        let row3 = |value: f64, timing: &str| {
            json!({
                "id": 3,
                "value": value,
                "driver": ["repeat 1 of inf, reverse", timing],
                "range": [0.0, 100.0],
                "nodes": [],
            })
        };
        let row2 = json!({
            "id": 2, "value": 0.5, "driver": [], "range": null, "nodes": [10, 11],
        });
        assert_eq!(
            payloads(&mut rx),
            [json!({
                "values": [row3(0.0, "timing 0 -> 100, 1000ms easeInOut, 0%"), row2],
                "total": 2,
            })],
            "opening the tab sends the table at once"
        );

        // A quarter through the timing; the next frame is inside `SAMPLE`.
        app.world_mut().resource_mut::<SharedValues>().tick(0.25);
        app.update();
        assert!(payloads(&mut rx).is_empty(), "sampled every SAMPLE");
        app.update();
        assert_eq!(
            payloads(&mut rx),
            [json!({
                "values": [row3(6.25, "timing 0 -> 100, 1000ms easeInOut, 25%"), row2],
                "total": 2,
            })],
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
}
