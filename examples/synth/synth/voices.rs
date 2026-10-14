//! The voice: the oscillator, noise, the LFO, the envelope and the filter.

use std::f32::consts::{PI, TAU};

use super::engine::{DT, Params, RATE};

/// xorshift32: white noise, the same every run.
pub struct Rng(pub u32);

impl Rng {
    /// -1..1.
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// One oscillator, or three a little apart once detuned (a supersaw), each
/// from its own phase so the unison never starts as one.
pub struct Osc {
    phase: [f32; 3],
}

impl Osc {
    pub fn new(rng: &mut Rng) -> Self {
        Self {
            phase: [0.0, rng.next().abs(), rng.next().abs()],
        }
    }

    /// `[wave, tune, level, detune]`: `pitch` adds two octaves per unit,
    /// `fm` two semitones.
    pub fn step(&mut self, p: &Params, t: &Params, pitch: f32, fm: f32) -> f32 {
        let semis = (t[1] * 24.0 - 12.0).round() + pitch * 24.0 + fm * 2.0;
        let hz = 110.0 * (semis / 12.0).exp2();
        // Up to a third of a semitone each way; the sides fade in first.
        let spread = (p[3] * p[3] * 0.35 / 12.0).exp2();
        let sides = (p[3] * 4.0).min(1.0);
        let voices = if sides > 0.0 { 3 } else { 1 };
        let mut sum = 0.0;
        for (k, (phase, ratio)) in self
            .phase
            .iter_mut()
            .zip([1.0, spread, 1.0 / spread])
            .enumerate()
            .take(voices)
        {
            let dt = (hz * ratio * DT).min(0.45);
            *phase = (*phase + dt).fract();
            sum += wave(t[0], *phase, dt) * if k == 0 { 1.0 } else { sides };
        }
        sum / (1.0 + 2.0 * sides * sides).sqrt() * p[2] * 0.5
    }
}

/// The waveforms, as the scopes draw them: sine, saw, square, triangle. The
/// saw and the square are band-limited (PolyBLEP) for `dt` (cycles per
/// sample).
fn wave(shape: f32, ph: f32, dt: f32) -> f32 {
    match shape as u32 {
        0 => (ph * TAU).sin(),
        1 => 2.0 * ph - 1.0 - blep(ph, dt),
        2 => {
            let square = if ph < 0.5 { 1.0 } else { -1.0 };
            (square + blep(ph, dt) - blep((ph + 0.5).fract(), dt)) * 0.8
        }
        _ => 1.0 - 4.0 * (ph - 0.5).abs(),
    }
}

/// The PolyBLEP correction around a discontinuity at phase 0.
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

pub struct Noise {
    rng: Rng,
    s: [f32; 3],
}

impl Noise {
    pub fn new(rng: Rng) -> Self {
        Self { rng, s: [0.0; 3] }
    }

    /// `[color, level]`: white, pink or brown.
    pub fn step(&mut self, p: &Params, t: &Params) -> f32 {
        let w = self.rng.next();
        let s = &mut self.s;
        let v = match t[0] as u32 {
            0 => w * 0.6,
            1 => {
                // Paul Kellet's economy pink.
                s[0] = 0.99765 * s[0] + w * 0.099_046;
                s[1] = 0.963 * s[1] + w * 0.296_516_4;
                s[2] = 0.57 * s[2] + w * 1.052_691_3;
                (s[0] + s[1] + s[2] + w * 0.1848) * 0.25
            }
            _ => {
                s[0] = (s[0] + 0.02 * w) / 1.02;
                s[0] * 4.0
            }
        };
        v * p[1] * 0.6
    }
}

pub struct Lfo {
    phase: f32,
    held: f32,
    out: f32,
    rng: Rng,
}

impl Lfo {
    pub fn new(rng: Rng) -> Self {
        Self {
            phase: 0.0,
            held: 0.0,
            out: 0.0,
            rng,
        }
    }

    /// `[shape, rate, depth]`: sine, triangle, square or a random step
    /// each cycle, 0.02 to 20 Hz.
    pub fn step(&mut self, p: &Params, t: &Params) -> f32 {
        self.phase += 0.02 * 1000f32.powf(p[1]) * DT;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.held = self.rng.next();
        }
        let v = match t[0] as u32 {
            0 => (self.phase * TAU).sin(),
            1 => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            2 => (self.phase < 0.5) as u8 as f32 * 2.0 - 1.0,
            _ => self.held,
        };
        // A jump slews over a couple of milliseconds: a cutoff it moves
        // never clicks.
        self.out += (v - self.out) * (DT / 0.002);
        self.out * p[2]
    }
}

#[derive(Default)]
pub struct Envelope {
    level: f32,
    stage: Stage,
    last: bool,
}

#[derive(Default)]
enum Stage {
    #[default]
    Release,
    Attack,
    Decay,
}

impl Envelope {
    /// `[a, d, s, r]`, opened by a gate. The attack charges like a
    /// capacitor toward a little past full (an analog envelope's curve);
    /// decay and release fall exponentially.
    pub fn step(&mut self, p: &Params, gate: f32) -> f32 {
        let on = gate > 0.5;
        if on && !self.last {
            self.stage = Stage::Attack;
        } else if !on {
            self.stage = Stage::Release;
        }
        self.last = on;
        let time = |k: f32, max: f32| 0.002 + k * k * max;
        match self.stage {
            Stage::Attack => {
                // From 0, 1.3 is reached through 1 at `time` (ln(1.3/0.3)).
                self.level += (1.3 - self.level) * toward(time(p[0], 2.0) / 1.466);
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => self.level += (p[2] - self.level) * toward(time(p[1], 2.0)),
            Stage::Release => self.level -= self.level * toward(time(p[3], 3.0)),
        }
        self.level
    }
}

/// The per-sample step of an exponential approach with time constant
/// `secs`.
fn toward(secs: f32) -> f32 {
    1.0 - (-DT / secs).exp()
}

/// A state-variable filter (Simper's trapezoidal form) whose band
/// saturates: a ringing filter grows hoarse, not ever louder.
#[derive(Default)]
pub struct Svf {
    s: [f32; 2],
}

/// Where the band saturates.
const SAT: f32 = 2.0;

impl Svf {
    /// `g` from [`cutoff`], `k` = 1 / Q; the low-, band- and highpass.
    pub fn run(&mut self, x: f32, g: f32, k: f32) -> [f32; 3] {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v3 = x - self.s[1];
        let band = SAT * ((a1 * self.s[0] + g * a1 * v3) / SAT).tanh();
        let low = self.s[1] + g * band;
        self.s[0] = 2.0 * band - self.s[0];
        self.s[1] = 2.0 * low - self.s[1];
        [low, band, x - k * band - low]
    }
}

/// The filter coefficient for a cutoff in Hz.
pub fn cutoff(hz: f32) -> f32 {
    (PI * hz.clamp(10.0, RATE as f32 * 0.45) * DT).tan()
}

#[derive(Default)]
pub struct Filter(Svf);

impl Filter {
    /// `[mode, cutoff, res, mod]`: low-, band- or highpass, its cutoff swept
    /// by `sweep` (up to six octaves, as far as `mod` opens it) and `fm`
    /// (two octaves per unit), resonance from none to ringing (Q 0.5 to
    /// 25).
    pub fn step(&mut self, p: &Params, t: &Params, x: f32, sweep: f32, fm: f32) -> f32 {
        let octaves = sweep * p[3] * 6.0 + fm * 2.0;
        let hz = 20.0 * 1000f32.powf(p[1]) * octaves.exp2();
        let q = 0.5 * 50f32.powf(p[2]);
        let [low, band, high] = self.0.run(x, cutoff(hz), 1.0 / q);
        match t[0] as u32 {
            0 => low,
            1 => band / q,
            _ => high,
        }
    }
}
