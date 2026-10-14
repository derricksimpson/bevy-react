//! The synth's sound: every module a few lines of DSP run once per sample,
//! upstream first, on the audio thread. A module reads its inputs from what
//! its sources produced this sample (or the last one, inside a feedback
//! loop) and holds its own state — a phase, a filter's memory, a delay
//! line — which survives knob turns and re-patching. The plumbing lives
//! here; the voice, the drums and the effects in their own files.

use std::sync::Arc;

use serde::Deserialize;
use ts_rs::TS;

use super::drums::Drum;
use super::effects::{Delay, Plate};
use super::voices::{Envelope, Filter, Lfo, Noise, Osc, Rng};

pub const RATE: u32 = 48_000;
pub const DT: f32 = 1.0 / RATE as f32;
/// Param slots per module (the sequencer's nine is the most).
pub const SLOTS: usize = 9;
/// Sockets per module, at most: the mixer's four inputs, the beat's four
/// outputs.
pub const INS: usize = 4;
pub const OUTS: usize = 4;
/// Knobs glide to a new setting over about this long, seconds — no zipper.
const GLIDE: f32 = 0.012;
/// Play and pause fade the whole patch in and out over this long, seconds.
const FADE: f32 = 0.06;
/// A re-wiring ducks the output this long each way, seconds, so a cable
/// plugged or pulled never pops.
const DUCK: f32 = 0.005;
/// Makeup gain into the output's soft clip.
const MASTER: f32 = 1.6;

pub type Params = [f32; SLOTS];

/// The modules, as `KINDS` names them in `ui/src/graph/catalog.ts`.
#[derive(Deserialize, TS, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Clock,
    Sequencer,
    Beat,
    Oscillator,
    Noise,
    Lfo,
    Drum,
    Envelope,
    Filter,
    Vca,
    Glide,
    Mixer,
    Delay,
    Reverb,
    Output,
}

impl Kind {
    /// The params the engine reads, in slot order (the catalog's names).
    pub fn params(self) -> &'static [&'static str] {
        match self {
            Kind::Clock => &["div", "swing"],
            Kind::Sequencer => &["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "mask"],
            Kind::Beat => &["k0", "k1", "k2", "k3"],
            Kind::Oscillator => &["wave", "tune", "level", "detune"],
            Kind::Noise => &["color", "level"],
            Kind::Lfo => &["shape", "rate", "depth"],
            Kind::Drum => &["voice", "tune", "decay", "level"],
            Kind::Envelope => &["a", "d", "s", "r"],
            Kind::Filter => &["mode", "cutoff", "res", "mod"],
            Kind::Vca => &["gain", "curve"],
            Kind::Glide => &["time"],
            Kind::Mixer => &["l0", "l1", "l2", "l3"],
            Kind::Delay => &["time", "feedback", "mix", "sync"],
            Kind::Reverb => &["size", "damp", "mix"],
            Kind::Output => &["volume"],
        }
    }
}

/// A module ready to run: wiring resolved to indexes on the main thread.
#[derive(Clone, Debug)]
pub struct Spec {
    pub id: String,
    pub kind: Kind,
    pub params: Params,
    /// Each input's source: (index in [`Patch::modules`], output port).
    pub inputs: [Option<(usize, usize)>; INS],
}

/// The whole patch, upstream modules first.
#[derive(Clone, Debug, Default)]
pub struct Patch {
    pub modules: Vec<Spec>,
    pub playing: bool,
    pub bpm: f32,
    /// A fingerprint of the cables: when it changes, the output ducks.
    pub wiring: u64,
}

/// The transport, as every module sees it this sample.
pub struct Ctx {
    /// Eighth notes since the transport started.
    pub steps: f64,
    pub playing: bool,
    pub bpm: f32,
}

struct Module {
    id: String,
    kind: Kind,
    target: Params,
    /// `target`, glided (the continuous knobs read this).
    params: Params,
    inputs: [Option<(usize, usize)>; INS],
    out: [f32; OUTS],
    dsp: Dsp,
}

/// Each kind's state.
enum Dsp {
    Clock,
    Sequencer(Steps),
    Beat(Steps),
    Oscillator(Osc),
    Noise(Noise),
    Lfo(Lfo),
    Drum(Drum),
    Envelope(Envelope),
    Filter(Filter),
    Vca,
    Glide(f32),
    Mixer,
    Delay(Delay),
    Reverb(Box<Plate>),
    Output,
}

impl Module {
    fn new(spec: &Spec) -> Self {
        // Any non-zero seed, different per module.
        let mut rng = Rng(spec
            .id
            .bytes()
            .fold(0x9e37_79b9_u32, |h, b| h.rotate_left(5) ^ b as u32)
            | 1);
        let dsp = match spec.kind {
            Kind::Clock => Dsp::Clock,
            Kind::Sequencer => Dsp::Sequencer(Steps::default()),
            Kind::Beat => Dsp::Beat(Steps::default()),
            Kind::Oscillator => Dsp::Oscillator(Osc::new(&mut rng)),
            Kind::Noise => Dsp::Noise(Noise::new(rng)),
            Kind::Lfo => Dsp::Lfo(Lfo::new(rng)),
            Kind::Drum => Dsp::Drum(Drum::new(rng)),
            Kind::Envelope => Dsp::Envelope(Envelope::default()),
            Kind::Filter => Dsp::Filter(Filter::default()),
            Kind::Vca => Dsp::Vca,
            Kind::Glide => Dsp::Glide(0.0),
            Kind::Mixer => Dsp::Mixer,
            Kind::Delay => Dsp::Delay(Delay::new()),
            Kind::Reverb => Dsp::Reverb(Box::new(Plate::new())),
            Kind::Output => Dsp::Output,
        };
        Self {
            id: spec.id.clone(),
            kind: spec.kind,
            target: spec.params,
            params: spec.params,
            inputs: spec.inputs,
            out: [0.0; OUTS],
            dsp,
        }
    }

    /// One sample: inputs in, outputs out. Each kind's params are its
    /// `Kind::params` in slot order; discrete ones (tabs, masks) read the
    /// target, continuous ones the glided value. `clock` is the division and
    /// swing of the clock feeding input 0, if a clock does.
    fn step(&mut self, ins: [f32; INS], ctx: &Ctx, clock: Option<(f32, f32)>) -> [f32; OUTS] {
        for (p, t) in self.params.iter_mut().zip(self.target) {
            *p += (t - *p) * (DT / GLIDE);
        }
        let (p, t) = (&self.params, &self.target);
        let patched = |i: usize| self.inputs[i].is_some();
        let mono = |v: f32| [v, 0.0, 0.0, 0.0];
        match &mut self.dsp {
            Dsp::Clock => mono(gate(ctx.playing && pulse(t[0], p[1], ctx.steps))),
            Dsp::Sequencer(s) => s.sequencer(t, patched(0) && ins[0] > 0.5, ctx.steps),
            Dsp::Beat(s) => s.beat(t, clock, patched(0) && ins[0] > 0.5, ctx),
            Dsp::Oscillator(osc) => mono(osc.step(p, t, ins[0], ins[1])),
            Dsp::Noise(noise) => mono(noise.step(p, t)),
            Dsp::Lfo(lfo) => mono(lfo.step(p, t)),
            Dsp::Drum(drum) => mono(drum.step(p, t, ins[0], ins[1])),
            Dsp::Envelope(env) => mono(env.step(p, ins[0])),
            Dsp::Filter(filter) => mono(filter.step(p, t, ins[0], ins[1], ins[2])),
            Dsp::Vca => {
                // `[gain, curve]`: an unpatched cv holds it open; the
                // exponential curve makes a fading cv fade faster.
                let cv = if patched(1) { ins[1].max(0.0) } else { 1.0 };
                let cv = if t[1] >= 0.5 { cv * cv * cv } else { cv };
                mono(ins[0] * cv * p[0])
            }
            Dsp::Glide(at) => {
                // `[time]`: the input, slewed (an RC's curve, up to half a
                // second).
                let secs = 0.001 + p[0] * p[0] * 0.5;
                *at += (ins[0] - *at) * (1.0 - (-DT / secs).exp());
                mono(*at)
            }
            Dsp::Mixer => mono(ins.iter().zip(p).map(|(x, level)| x * level).sum::<f32>() * 0.7),
            Dsp::Delay(delay) => mono(delay.step(p, t, ins[0], ctx.bpm)),
            Dsp::Reverb(plate) => {
                let [left, right] = plate.step(p, ins[0]);
                [left, right, 0.0, 0.0]
            }
            Dsp::Output => {
                // The right input follows the left until it is patched.
                let right = if patched(1) { ins[1] } else { ins[0] };
                [ins[0] * p[0], right * p[0], 0.0, 0.0]
            }
        }
    }
}

fn gate(on: bool) -> f32 {
    on as u8 as f32
}

/// Whether the clock's pulse is high: one per quarter, eighth or sixteenth
/// note (`div`), every other one `swing` late — a third of the way to the
/// next at most, a triplet shuffle.
fn pulse(div: f32, swing: f32, steps: f64) -> bool {
    let period = [2.0, 1.0, 0.5][(div as usize).min(2)];
    let at = (steps / (2.0 * period)).fract();
    let late = 0.5 + swing as f64 / 6.0;
    at < 0.25 || (late..late + 0.25).contains(&at)
}

/// Sixteenths since the transport started, as a clock of `div` and `swing`
/// plays them: its pulses land on their sixteenths (every other one late,
/// as [`pulse`] has it), and the sixteenths between a slower clock's pulses
/// split the gap evenly.
fn felt(div: f32, swing: f32, steps: f64) -> f64 {
    // Two pulses, in sixteenths: quarter, eighth or sixteenth ones.
    let pair = [8.0, 4.0, 2.0][(div as usize).min(2)];
    let s = steps * 2.0;
    let start = (s / pair).floor() * pair;
    let (at, half) = (s - start, pair / 2.0);
    let late = pair * (0.5 + swing as f64 / 6.0);
    start
        + if at < late {
            at * half / late
        } else {
            half + (at - late) * half / (pair - late)
        }
}

/// The sequencer's and the beat's state: the clock's last level, the gates
/// out, the held pitch; the beat's last sixteenth and where it struck.
#[derive(Default)]
struct Steps {
    last: bool,
    gates: [bool; OUTS],
    pitch: f32,
    at: Option<i64>,
    from: f64,
}

impl Steps {
    /// `[p0..p7, mask]`: on each clock pulse, the step the transport is on
    /// plays (if it isn't muted) — the step the lights show. Its gate is
    /// high as long as the pulse.
    fn sequencer(&mut self, t: &Params, clock: bool, steps: f64) -> [f32; OUTS] {
        if clock && !self.last {
            let step = (steps.floor() as i64).rem_euclid(8) as usize;
            self.gates[0] = (t[8] as u32 >> step) & 1 == 1;
            if self.gates[0] {
                self.pitch = quantize(t[step]);
            }
        } else if !clock {
            self.gates[0] = false;
        }
        self.last = clock;
        [self.pitch, gate(self.gates[0]), 0.0, 0.0]
    }

    /// `[k0..k3]`, a bar of sixteenths per lane: every lane with a pad on
    /// the sixteenth fires. On a clock, the beat plays all sixteen in the
    /// clock's time (a slower clock's pulses split, its swing kept), each
    /// gate half a sixteenth long; on any other gate, the sixteenth each
    /// rising edge lands on, as long as the gate.
    fn beat(
        &mut self,
        t: &Params,
        clock: Option<(f32, f32)>,
        input: bool,
        ctx: &Ctx,
    ) -> [f32; OUTS] {
        let sixteenth = ctx.steps * 2.0;
        let (hit, off) = match clock {
            Some((div, swing)) => {
                let n = felt(div, swing, ctx.steps).floor() as i64;
                let hit = ctx.playing && self.at != Some(n);
                self.at = ctx.playing.then_some(n);
                (
                    hit.then_some(n),
                    !ctx.playing || sixteenth - self.from >= 0.5,
                )
            }
            None => {
                let hit = input && !self.last;
                self.last = input;
                (hit.then(|| sixteenth.floor() as i64), !input)
            }
        };
        if let Some(n) = hit {
            self.from = sixteenth;
            for (on, lane) in self.gates.iter_mut().zip(t) {
                *on = (*lane as u32 >> n.rem_euclid(16)) & 1 == 1;
            }
        } else if off {
            self.gates = [false; OUTS];
        }
        self.gates.map(gate)
    }
}

/// A step's height as pitch CV: the nearest note of two octaves of minor
/// pentatonic, so any melody the bars draw sounds in key.
fn quantize(v: f32) -> f32 {
    const SCALE: [f32; 5] = [0.0, 3.0, 5.0, 7.0, 10.0];
    let n = (v.clamp(0.0, 1.0) * 10.0).round() as usize;
    (12.0 * (n / 5) as f32 + SCALE[n % 5]) / 24.0
}

/// The running patch.
pub struct Engine {
    modules: Vec<Module>,
    playing: bool,
    bpm: f32,
    /// Eighth notes since the transport started.
    steps: f64,
    gain: f32,
    wiring: u64,
    /// A re-wired patch waiting for the output to duck, and the duck.
    next: Option<Arc<Patch>>,
    duck: f32,
    /// The DC blocker's memory, per channel: last input, last output.
    dc: [[f32; 2]; 2],
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            modules: Vec::new(),
            playing: false,
            bpm: 120.0,
            steps: 0.0,
            gain: 0.0,
            wiring: 0,
            next: None,
            duck: 1.0,
            dc: [[0.0; 2]; 2],
        }
    }
}

impl Engine {
    /// Take in a new patch: at once, unless its cables changed while it
    /// sounds — then the output ducks first.
    pub fn post(&mut self, patch: Arc<Patch>) {
        if patch.wiring != self.wiring && self.gain > 0.0 {
            self.next = Some(patch);
        } else {
            self.next = None;
            self.apply(&patch);
        }
    }

    /// Take in a new patch now. A module that is still there keeps its
    /// state (and its knobs glide to their new settings); the transport
    /// restarts with React's step lights.
    pub fn apply(&mut self, patch: &Patch) {
        let mut old = std::mem::take(&mut self.modules);
        self.modules = patch
            .modules
            .iter()
            .map(|spec| {
                match old
                    .iter()
                    .position(|m| m.id == spec.id && m.kind == spec.kind)
                {
                    Some(i) => Module {
                        target: spec.params,
                        inputs: spec.inputs,
                        ..old.swap_remove(i)
                    },
                    None => Module::new(spec),
                }
            })
            .collect();
        if patch.playing && (!self.playing || patch.bpm != self.bpm) {
            self.steps = 0.0;
        }
        self.playing = patch.playing;
        self.bpm = patch.bpm;
        self.wiring = patch.wiring;
    }

    /// Where the transport is: eighth notes since it started, whether it
    /// runs, its tempo.
    pub fn transport(&self) -> (f64, bool, f32) {
        (self.steps, self.playing, self.bpm)
    }

    /// Fill `out` with interleaved stereo.
    pub fn render(&mut self, out: &mut [f32]) {
        for frame in out.as_chunks_mut::<2>().0 {
            if self.next.is_some() {
                self.duck -= DT / DUCK;
                if self.duck <= 0.0 {
                    self.duck = 0.0;
                    if let Some(next) = self.next.take() {
                        self.apply(&next);
                    }
                }
            } else {
                self.duck = (self.duck + DT / DUCK).min(1.0);
            }
            self.gain = if self.playing {
                (self.gain + DT / FADE).min(1.0)
            } else {
                (self.gain - DT / FADE).max(0.0)
            };
            if self.gain == 0.0 {
                frame.fill(0.0);
                continue;
            }
            if self.playing {
                self.steps += self.bpm as f64 / 30.0 / RATE as f64;
            }
            let ctx = Ctx {
                steps: self.steps,
                playing: self.playing,
                bpm: self.bpm,
            };
            let mut mix = [0.0f32; 2];
            for i in 0..self.modules.len() {
                let mut ins = [0.0; INS];
                for (v, src) in ins.iter_mut().zip(self.modules[i].inputs) {
                    if let Some((m, port)) = src {
                        *v = self.modules[m].out[port];
                    }
                }
                // A beat plays in the time of the clock that drives it.
                let clock = match self.modules[i].inputs[0] {
                    Some((m, _)) if self.modules[m].kind == Kind::Clock => {
                        Some((self.modules[m].target[0], self.modules[m].params[1]))
                    }
                    _ => None,
                };
                let module = &mut self.modules[i];
                module.out = module.step(ins, &ctx, clock);
                if module.kind == Kind::Output {
                    mix[0] += module.out[0];
                    mix[1] += module.out[1];
                }
            }
            for ((sample, x), dc) in frame.iter_mut().zip(mix).zip(&mut self.dc) {
                // Fade, then block DC (below a few Hz: a kick keeps its
                // weight) — in that order, so a patch cut under the duck
                // leaves no step behind — then a soft clip: a loud patch
                // saturates instead of cracking.
                let x = x * self.gain * self.duck;
                let y = x - dc[0] + 0.9995 * dc[1];
                *dc = [x, y];
                *sample = (y * MASTER).tanh();
            }
        }
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
