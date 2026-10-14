//! The Animations tab: the shared-value table — every running driver, plus
//! every value an app node is bound to — and every transition in flight
//! (any [`AnimationRuns`](crate::ext::AnimationRuns) component's runs),
//! streamed to the panel with each one's live state and timeline. While the
//! tab pauses the app ([`clock`]), the panel scrubs any of them.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use bevy::prelude::*;

use crate::animations::{AnimatedNode, SharedId, SharedValues};
use crate::bridge::{JsBridge, ReactNode};
use crate::event::ReactEvents;
use crate::ext::AnimationRunSources;
use crate::protocol::NodeId;
use crate::reconcile::climb;
use crate::{react_event, react_message};

use super::DevtoolsState;
use super::layers::round3;

mod clock;
mod describe;
#[cfg(test)]
mod tests;

pub(super) use clock::{TakenClock, on_clock_message, pause_on_key, restore_clock};
use describe::{describe, span, spec, timeline};

/// How often the stream samples. The readouts are numbers to read, not to
/// watch move — the panel's meters are bound to the shared values
/// themselves, so the engine moves those every frame with no stream at all.
const SAMPLE: Duration = Duration::from_millis(100);

/// ponytail: rows past the cap are counted, not listed; window the list if
/// an app ever needs thousands inspected at once. Per section.
const MAX_ROWS: usize = 200;

/// Bevy → JS: the listed shared values (running drivers first, then by id)
/// and the nodes with transitions in flight (by node id). Streamed by
/// [`emit_animations`] only while the Animations tab is shown, sampled every
/// [`SAMPLE`] and diffed against the last payload, so an idle app sends
/// nothing.
#[react_event(name = "devtools.animations")]
#[derive(Clone, PartialEq)]
pub(super) struct DevtoolsAnimations {
    values: Vec<DevtoolsAnimationRow>,
    /// How many values qualified before the [`MAX_ROWS`] cap.
    total: u32,
    transitions: Vec<DevtoolsTransitionRow>,
    /// How many nodes qualified before the cap.
    transitions_total: u32,
    /// The app clock (`Time<Virtual>`), which the tab's controls drive.
    clock: DevtoolsClock,
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
    /// The running driver's scrubber stretch, `[at, start, end]` seconds.
    time: Option<[f64; 3]>,
    /// The app nodes bound to this value (the panel's own never count).
    nodes: Vec<NodeId>,
}

/// One node's transitions in flight.
#[derive(serde::Serialize, ts_rs::TS, Debug, Clone, PartialEq)]
struct DevtoolsTransitionRow {
    node: NodeId,
    lines: Vec<DevtoolsRunLine>,
}

/// One line of a transition row: the runs a label groups.
#[derive(serde::Serialize, ts_rs::TS, Debug, Clone, PartialEq)]
struct DevtoolsRunLine {
    /// The `transition` key (or the feature's label) that armed it.
    label: String,
    /// Its timing, as the spec reads (see [`spec`]).
    spec: String,
    /// The scrubber stretch, `[at, start, end]` seconds.
    time: [f64; 3],
}

#[derive(serde::Serialize, ts_rs::TS, Debug, Clone, PartialEq)]
struct DevtoolsClock {
    paused: bool,
    speed: f64,
}

impl DevtoolsClock {
    fn of(clock: &Time<Virtual>) -> Self {
        Self {
            paused: clock.is_paused(),
            speed: round3(clock.relative_speed()),
        }
    }
}

/// JS → Bevy: the panel's Animations tab was shown/hidden. Gates the stream
/// (and its unmount hands the clock back — [`restore_clock`]).
#[react_message(name = "devtools.animationsOpen")]
pub(super) struct DevtoolsAnimationsOpenMessage {
    pub(super) on: bool,
}

/// JS → Bevy: move a shared value's running driver to `t` seconds since it
/// started.
#[react_message(name = "devtools.seekValue")]
pub(super) struct DevtoolsSeekValueMessage {
    id: SharedId,
    t: f32,
}

/// JS → Bevy: move a node's runs under `label` to `t` seconds since each
/// started.
#[react_message(name = "devtools.seekRun")]
pub(super) struct DevtoolsSeekRunMessage {
    node: NodeId,
    label: String,
    t: f32,
}

pub(super) fn on_animations_open_message(
    msg: On<DevtoolsAnimationsOpenMessage>,
    mut state: ResMut<DevtoolsState>,
) {
    state.animations_tab_open = msg.event().on;
}

pub(super) fn on_seek_value_message(
    msg: On<DevtoolsSeekValueMessage>,
    mut values: ResMut<SharedValues>,
) {
    values.seek(msg.event().id, msg.event().t);
}

/// Seeks need the world (the run sources visit it), so the move is a
/// queued command.
pub(super) fn on_seek_run_message(msg: On<DevtoolsSeekRunMessage>, mut commands: Commands) {
    let (node, label, t) = (msg.event().node, msg.event().label.clone(), msg.event().t);
    commands.queue(move |world: &mut World| {
        let entity = world
            .get_resource::<JsBridge>()
            .and_then(|b| b.nodes.get(&node).copied());
        let sources = world
            .get_resource::<AnimationRunSources>()
            .map(AnimationRunSources::sources);
        let (Some(entity), Some(sources)) = (entity, sources) else {
            return;
        };
        for source in sources {
            (source.one)(world, entity, &mut |l, run| {
                if l == label {
                    run.seek(t);
                }
            });
        }
    });
}

/// The stream's sampler: when it is due, and the transition rows the
/// collector gathered for it.
#[derive(Resource, Default)]
pub(super) struct AnimationsSample {
    since: Duration,
    due: bool,
    /// The last payload sent; `None` = send the next one whatever it holds.
    last: Option<DevtoolsAnimations>,
    transitions: Vec<DevtoolsTransitionRow>,
    transitions_total: u32,
}

pub(super) fn sample_due(sample: Res<AnimationsSample>) -> bool {
    sample.due
}

/// Decide whether this frame samples: every [`SAMPLE`] of real time (a
/// paused app still streams), and at once when the tab was just shown or
/// the clock changed (the panel's controls show the stream's clock, so a
/// click or the pause key reads back within a frame).
pub(super) fn gate_animations_sample(
    state: Res<DevtoolsState>,
    time: Res<Time<Real>>,
    clock: Res<Time<Virtual>>,
    mut sample: ResMut<AnimationsSample>,
) {
    if !(state.open && state.animations_tab_open) {
        // A re-shown tab gets a fresh full payload.
        if sample.due || sample.last.is_some() {
            sample.due = false;
            sample.last = None;
        }
        return;
    }
    sample.since += time.delta();
    let clock_moved = sample
        .last
        .as_ref()
        .is_some_and(|last| last.clock != DevtoolsClock::of(&clock));
    sample.due = sample.last.is_none() || clock_moved || sample.since >= SAMPLE;
    if sample.due {
        sample.since = Duration::ZERO;
    }
}

/// Gather every node's runs in flight from the registered
/// [`AnimationRuns`](crate::ext::AnimationRuns) types, one row per node,
/// one line per label (the first run visited gives a line its timeline).
pub(super) fn collect_transitions(world: &mut World) {
    let Some(sources) = world
        .get_resource::<AnimationRunSources>()
        .map(AnimationRunSources::sources)
    else {
        return;
    };
    let mut seen = Vec::new();
    for source in sources {
        (source.all)(world, &mut |entity, label, run| {
            seen.push((entity, label, timeline(run), spec(run.driver())));
        });
    }
    let mut rows: BTreeMap<NodeId, Vec<DevtoolsRunLine>> = BTreeMap::new();
    for (entity, label, time, spec) in seen {
        let Some(node) = world.get::<ReactNode>(entity) else {
            continue;
        };
        let lines = rows.entry(node.0).or_default();
        if lines.iter().all(|l| l.label != label) {
            lines.push(DevtoolsRunLine {
                label: label.into(),
                spec,
                time,
            });
        }
    }
    let mut sample = world.resource_mut::<AnimationsSample>();
    sample.transitions_total = rows.len() as u32;
    sample.transitions = rows
        .into_iter()
        .take(MAX_ROWS)
        .map(|(node, lines)| DevtoolsTransitionRow { node, lines })
        .collect();
}

/// Push `devtools.animations` on a sampled frame. Lists every value with a
/// running driver and every value an app node reads (an idle, unbound value
/// is noise). Nodes under the panel's own `<root>` are skipped: its meters
/// bind the values it lists, which would keep a finished value listed
/// forever.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_animations(
    state: Res<DevtoolsState>,
    values: Res<SharedValues>,
    clock: Res<Time<Virtual>>,
    bridge: Option<Res<JsBridge>>,
    animated: Query<(Entity, &ReactNode, &AnimatedNode)>,
    child_of: Query<&ChildOf>,
    events: ReactEvents,
    mut sample: ResMut<AnimationsSample>,
) {
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
        .filter(|(id, _, run)| run.is_some() || bound.contains_key(id))
        .collect();
    listed.sort_by_key(|&(id, _, run)| (run.is_none(), id));
    let payload = DevtoolsAnimations {
        total: listed.len() as u32,
        values: listed
            .into_iter()
            .take(MAX_ROWS)
            .map(|(id, value, run)| {
                let mut nodes = bound.remove(&id).unwrap_or_default();
                nodes.sort_unstable();
                nodes.dedup();
                let mut lines = Vec::new();
                if let Some(run) = run {
                    describe(run.runner(), &mut lines);
                }
                DevtoolsAnimationRow {
                    id,
                    value: round3(value),
                    driver: lines,
                    range: run.and_then(|run| span(run.runner(), run.from())),
                    time: run.map(timeline),
                    nodes,
                }
            })
            .collect(),
        transitions: std::mem::take(&mut sample.transitions),
        transitions_total: sample.transitions_total,
        clock: DevtoolsClock::of(&clock),
    };
    if sample.last.as_ref() != Some(&payload) {
        events.send(&payload);
        sample.last = Some(payload);
    }
}
