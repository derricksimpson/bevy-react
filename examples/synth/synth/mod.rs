//! The patch, audible. React sends what the patch is made of (`synth.set`:
//! the modules, their knobs, the cables, the transport) whenever that
//! changes — never on a move — and this module resolves it into a running
//! order and hands it to the audio thread, where [`engine`] plays it one
//! sample at a time. The audio thread picks up the latest patch at the top
//! of each block without ever waiting for the main thread, and tells it
//! where its transport is: the sound is the clock the step lights and the
//! wires' pulses show ([`Transport`]).

mod drums;
mod effects;
mod engine;
mod voices;

use std::collections::{HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source};
// `std`'s panics on the web.
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy_react::animations::{AnimationSet, SharedId, SharedValues};
use bevy_react::{ReactAppExt, react_message};
use serde::Deserialize;
use ts_rs::TS;

use engine::{Engine, INS, Kind, OUTS, Patch, RATE, SLOTS, Spec};

/// One end of a cable: a module and the index of its socket.
#[derive(Deserialize, TS, Clone, Debug)]
pub struct Jack {
    pub node: String,
    pub port: u32,
}

/// A cable, from an output to an input.
#[derive(Deserialize, TS, Clone, Debug)]
pub struct Cable {
    pub from: Jack,
    pub to: Jack,
}

#[derive(Deserialize, TS, Clone, Debug)]
pub struct Module {
    pub id: String,
    pub kind: Kind,
    /// The catalog's params by name; the engine reads the ones it knows.
    pub params: HashMap<String, f32>,
}

/// React → Bevy: what the patch is made of.
#[react_message(name = "synth.set")]
pub struct SetSynth {
    pub modules: Vec<Module>,
    pub cables: Vec<Cable>,
    pub playing: bool,
    pub bpm: f32,
    /// The shared value React's step lights read (its `steps.id`): Bevy
    /// drives it from the sound.
    pub lights: u32,
}

/// Frames rendered per block: the audio thread checks for a new patch this
/// often (about 5 ms).
const BLOCK: usize = 256;

/// The latest patch and how many have been posted.
type Mailbox = Mutex<(u64, Arc<Patch>)>;

#[derive(Resource)]
struct Post(Arc<Mailbox>);

/// Where the audio thread's transport was when the device last asked for
/// sound, and how long that sound waits to be heard.
#[derive(Clone, Copy)]
struct Rendered {
    steps: f64,
    playing: bool,
    bpm: f32,
    at: Instant,
    /// The device's period: it asks for the next one as it starts playing
    /// the last, so what it gets now plays a period later.
    lag: f64,
}

/// The audio thread writes it on each request, never waiting.
#[derive(Resource, Clone, Default)]
struct Heard(Arc<Mutex<Option<Rendered>>>);

/// The transport as the screen shows it — React's step lights (the shared
/// value `lights`) and the wires' pulses: where the sound is, or with no
/// sound (`--shoot`) a clock of its own.
#[derive(Resource, Default)]
pub struct Transport {
    /// Eighth notes into the two-bar loop.
    pub steps: f32,
    playing: bool,
    bpm: f32,
    lights: Option<SharedId>,
}

pub struct SynthPlugin {
    /// Off for `--shoot`: captures stay silent.
    pub audible: bool,
}

impl Plugin for SynthPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        let mailbox = Arc::new(Mailbox::default());
        let heard = Heard::default();
        app.add_audio_source::<Live>()
            .insert_resource(Post(mailbox.clone()))
            .insert_resource(heard.clone())
            .init_resource::<Transport>()
            .add_systems(
                Update,
                show_transport
                    .after(AnimationSet::Tick)
                    .before(AnimationSet::Apply),
            );
        if self.audible {
            app.add_systems(
                Startup,
                move |mut commands: Commands, mut live: ResMut<Assets<Live>>| {
                    let live = live.add(Live(mailbox.clone(), heard.clone()));
                    commands.spawn(AudioPlayer(live));
                },
            );
        }
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_set);
}

fn on_set(set: On<SetSynth>, post: Res<Post>, mut transport: ResMut<Transport>) {
    let ev = set.event();
    // The screen's own clock restarts with the sound's (on play and on a
    // new tempo); it shows only while nothing is heard.
    if ev.playing && (!transport.playing || ev.bpm != transport.bpm) {
        transport.steps = 0.0;
    }
    transport.playing = ev.playing;
    transport.bpm = ev.bpm;
    transport.lights = Some(ev.lights);
    let patch = Arc::new(resolve(ev));
    debug!(
        "synth: {} modules, {} cables, playing {}",
        patch.modules.len(),
        set.event().cables.len(),
        patch.playing
    );
    let mut mailbox = post.0.lock().unwrap();
    *mailbox = (mailbox.0 + 1, patch);
}

/// Where the sound is, in eighth notes into the two-bar loop: the rendered
/// transport aged to now, less the wait to be heard (a restart shows its
/// downbeat until it is).
fn heard_steps(r: &Rendered, now: Instant) -> f32 {
    let mut steps = r.steps;
    if r.playing {
        let age = now.saturating_duration_since(r.at).as_secs_f64() - r.lag;
        steps = (steps + age * r.bpm as f64 / 30.0).max(0.0);
    }
    (steps % 16.0) as f32
}

/// Every frame, before the animations apply: the step lights and the
/// wires' pulses show where the sound is — or, while no audio thread
/// renders (`--shoot`, no device), a clock of their own.
fn show_transport(
    time: Res<Time>,
    heard: Res<Heard>,
    mut transport: ResMut<Transport>,
    mut values: ResMut<SharedValues>,
) {
    let now = Instant::now();
    let rendered = heard.0.lock().ok().and_then(|r| *r);
    let t = &mut *transport;
    match rendered.filter(|r| now.saturating_duration_since(r.at) < Duration::from_millis(500)) {
        Some(r) => t.steps = heard_steps(&r, now),
        None if t.playing => {
            t.steps = (t.steps + time.delta_secs() * t.bpm.max(1.0) / 30.0) % 16.0;
        }
        None => {}
    }
    if let Some(id) = t.lights
        && values.get(id) != Some(t.steps)
    {
        values.set(id, t.steps);
    }
}

/// The message as the engine runs it: params in slots, cables as indexes,
/// modules upstream first. What is left of a feedback loop runs in any
/// order and reads its sources' previous sample. A cable to or from a
/// socket a module doesn't have is dropped.
fn resolve(set: &SetSynth) -> Patch {
    let index: HashMap<&str, usize> = set
        .modules
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.as_str(), i))
        .collect();
    let mut inputs = vec![[None; INS]; set.modules.len()];
    let mut wiring = Vec::new();
    for cable in &set.cables {
        let (Some(&from), Some(&to)) = (
            index.get(cable.from.node.as_str()),
            index.get(cable.to.node.as_str()),
        ) else {
            continue;
        };
        let port = cable.from.port as usize;
        if let Some(slot) = inputs[to].get_mut(cable.to.port as usize)
            && port < OUTS
        {
            *slot = Some((from, port));
            wiring.push((&cable.from.node, port, &cable.to.node, cable.to.port));
        }
    }
    wiring.sort();
    let mut hasher = DefaultHasher::new();
    wiring.hash(&mut hasher);

    // Kahn's algorithm over "feeds".
    let n = set.modules.len();
    let mut waiting: Vec<usize> = inputs
        .iter()
        .map(|ins| ins.iter().flatten().count())
        .collect();
    let mut ready: VecDeque<usize> = (0..n).filter(|&i| waiting[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    let mut placed = vec![false; n];
    while order.len() < n {
        // Stuck in a feedback loop: cut it where the most inputs are ready.
        let next = ready.pop_front().or_else(|| {
            (0..n)
                .filter(|&i| !placed[i])
                .max_by_key(|&i| (inputs[i].iter().flatten().count() - waiting[i], n - i))
        });
        let Some(i) = next else { break };
        if placed[i] {
            continue;
        }
        placed[i] = true;
        order.push(i);
        for (j, ins) in inputs.iter().enumerate() {
            for _ in ins.iter().flatten().filter(|(from, _)| *from == i) {
                waiting[j] -= 1;
                if waiting[j] == 0 && !placed[j] {
                    ready.push_back(j);
                }
            }
        }
    }

    let mut rank = vec![0; n];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    let modules = order
        .iter()
        .map(|&i| {
            let m = &set.modules[i];
            let mut params = [0.0; SLOTS];
            for (slot, name) in params.iter_mut().zip(m.kind.params()) {
                *slot = m.params.get(*name).copied().unwrap_or(0.0);
            }
            Spec {
                id: m.id.clone(),
                kind: m.kind,
                params,
                inputs: inputs[i].map(|src| src.map(|(from, port)| (rank[from], port))),
            }
        })
        .collect();
    Patch {
        modules,
        playing: set.playing,
        bpm: set.bpm,
        wiring: hasher.finish(),
    }
}

/// The synth as bevy_audio plays it: a never-ending source fed by the
/// mailbox.
#[derive(Asset, TypePath, Clone)]
struct Live(Arc<Mailbox>, Heard);

impl Decodable for Live {
    type Decoder = Voice;

    fn decoder(&self) -> Voice {
        Voice {
            mailbox: self.0.clone(),
            heard: self.1.clone(),
            asked: Instant::now(),
            rendered: Instant::now(),
            seen: 0,
            engine: Engine::default(),
            block: vec![0.0; BLOCK * 2],
            at: BLOCK * 2,
        }
    }
}

struct Voice {
    mailbox: Arc<Mailbox>,
    heard: Heard,
    /// When the device last asked for sound, and when a block was last
    /// rendered.
    asked: Instant,
    rendered: Instant,
    /// The last patch taken from the mailbox.
    seen: u64,
    engine: Engine,
    block: Vec<f32>,
    at: usize,
}

impl Iterator for Voice {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.at == self.block.len() {
            // Never wait for the main thread: if it holds the lock, the
            // new patch is taken a block later.
            let posted = self.mailbox.try_lock().ok().and_then(|mailbox| {
                (mailbox.0 != self.seen).then(|| (mailbox.0, mailbox.1.clone()))
            });
            if let Some((seen, patch)) = posted {
                self.seen = seen;
                self.engine.post(patch);
            }
            // The device pulls a period at a time, a burst of blocks: one
            // after a pause starts its next request.
            let now = Instant::now();
            if now - self.rendered > Duration::from_millis(2) {
                let lag = (now - self.asked).as_secs_f64().min(0.2);
                self.asked = now;
                if let Ok(mut heard) = self.heard.0.try_lock() {
                    let (steps, playing, bpm) = self.engine.transport();
                    *heard = Some(Rendered {
                        steps,
                        playing,
                        bpm,
                        at: now,
                        lag,
                    });
                }
            }
            self.engine.render(&mut self.block);
            self.rendered = Instant::now();
            self.at = 0;
        }
        self.at += 1;
        Some(self.block[self.at - 1])
    }
}

impl Source for Voice {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(id: &str, kind: Kind) -> Module {
        Module {
            id: id.into(),
            kind,
            params: HashMap::new(),
        }
    }

    fn cable(from: &str, to: &str, port: u32) -> Cable {
        Cable {
            from: Jack {
                node: from.into(),
                port: 0,
            },
            to: Jack {
                node: to.into(),
                port,
            },
        }
    }

    /// Sources run before what they feed, whatever order React lists them
    /// in, and a feedback loop still places every module once.
    #[test]
    fn upstream_runs_first() {
        let patch = resolve(&SetSynth {
            modules: vec![
                module("out", Kind::Output),
                module("mix", Kind::Mixer),
                module("dly", Kind::Delay),
                module("osc", Kind::Oscillator),
            ],
            cables: vec![
                cable("osc", "mix", 0),
                cable("dly", "mix", 1),
                cable("mix", "dly", 0),
                cable("mix", "out", 0),
            ],
            playing: true,
            bpm: 120.0,
            lights: 0,
        });
        let ids: Vec<&str> = patch.modules.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids.len(), 4);
        let at = |id| ids.iter().position(|i| *i == id).unwrap();
        assert!(at("osc") < at("mix") && at("mix") < at("out"));
        // The output's input points at the mixer's new place.
        assert_eq!(patch.modules[at("out")].inputs[0], Some((at("mix"), 0)));
    }

    /// The lights show where the sound is heard: the device's period after
    /// it took the sound, a restart's downbeat until the restart is heard,
    /// a paused transport where it stopped.
    #[test]
    fn the_lights_show_what_is_heard() {
        let at = Instant::now();
        let r = Rendered {
            steps: 15.5,
            playing: true,
            bpm: 120.0,
            at,
            lag: 0.05,
        };
        // Four eighths a second: 15.5 + 0.25 × 4, round the loop.
        let later = heard_steps(&r, at + Duration::from_millis(300));
        assert!((later - 0.5).abs() < 1e-3, "{later}");
        let restarted = Rendered { steps: 0.01, ..r };
        assert_eq!(heard_steps(&restarted, at), 0.0);
        let paused = Rendered {
            steps: 37.5,
            playing: false,
            ..r
        };
        assert_eq!(heard_steps(&paused, at + Duration::from_secs(1)), 5.5);
    }

    /// The audio thread tells the screen where its transport is when the
    /// device comes back for more, and how long it was away.
    #[test]
    fn a_request_reports_the_transport() {
        let patch = Patch {
            playing: true,
            bpm: 120.0,
            ..Patch::default()
        };
        let mailbox = Arc::new(Mailbox::new((1, Arc::new(patch))));
        let heard = Heard::default();
        let mut voice = Live(mailbox, heard.clone()).decoder();
        // A period's burst of blocks, a pause, the next request.
        voice.by_ref().take(BLOCK * 2 * 8).for_each(drop);
        assert!(heard.0.lock().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(10));
        voice.next();
        let r = heard.0.lock().unwrap().unwrap();
        assert!(
            r.playing && r.steps > 0.0 && r.lag >= 0.01,
            "{} {}",
            r.steps,
            r.lag
        );
    }

    /// Every module the catalog lists runs here, reading the params the
    /// catalog gives it, by name.
    #[test]
    fn params_match_the_catalog() {
        let ts = include_str!("../ui/src/graph/catalog.ts");
        let mut lines = ts
            .split("export const KINDS = {")
            .nth(1)
            .and_then(|rest| rest.split("} satisfies").next())
            .unwrap()
            .lines();
        let (mut kind, mut seen) = (None, 0);
        while let Some(line) = lines.next() {
            if let Some(name) = line.strip_prefix("  ").and_then(|l| l.strip_suffix(": {"))
                && !name.starts_with(' ')
            {
                let named = serde_json::from_str::<Kind>(&format!("\"{name}\""));
                kind = Some(named.unwrap_or_else(|_| panic!("the engine has no {name:?}")));
            }
            let Some(rest) = line.trim_start().strip_prefix("params: {") else {
                continue;
            };
            let mut body = rest.to_string();
            while !body.contains('}') {
                body += lines.next().unwrap();
            }
            let mut keys: Vec<&str> = body
                .split('}')
                .next()
                .unwrap()
                .split(',')
                .filter_map(|pair| pair.split(':').next())
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .collect();
            let kind = kind.unwrap();
            let mut want = kind.params().to_vec();
            keys.sort_unstable();
            want.sort_unstable();
            assert_eq!(keys, want, "{kind:?}");
            seen += 1;
        }
        assert!(seen > 0, "no kinds found in catalog.ts");
    }
}
