//! The drum voices, each struck by a rising gate (and transposed by a pitch
//! input, held at the hit) and synthesized from scratch: a kick (a sine swept down from a click), a snare (two tones
//! under a band of noise), a clap (three slaps of band-passed noise, then the
//! room) and a hat (six detuned squares, the TR-808's metal, band-passed
//! high).

use std::f32::consts::TAU;

use super::engine::{DT, Params};
use super::voices::{Rng, Svf, cutoff};

/// The hat's six squares, Hz (the TR-808's).
const METAL: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

pub struct Drum {
    /// Seconds since the last hit.
    t: f32,
    /// Still sounding.
    live: bool,
    last: bool,
    phase: [f32; 6],
    /// The pitch input as a frequency ratio, taken at the hit.
    ratio: f32,
    rng: Rng,
    svf: Svf,
}

impl Drum {
    pub fn new(rng: Rng) -> Self {
        Self {
            t: 0.0,
            live: false,
            last: false,
            phase: [0.0; 6],
            ratio: 1.0,
            rng,
            svf: Svf::default(),
        }
    }

    /// `[voice, tune, decay, level]`; `pitch` adds two octaves per unit,
    /// as an oscillator's does, held from each hit to the next.
    pub fn step(&mut self, p: &Params, t: &Params, trig: f32, pitch: f32) -> f32 {
        let voice = t[0] as u32;
        let on = trig > 0.5;
        if on && !self.last {
            self.t = 0.0;
            self.live = true;
            self.ratio = (pitch * 2.0).exp2();
            // The tones start the same every hit; the hat's metal runs free.
            if voice != 3 {
                self.phase = [0.0; 6];
            }
        }
        self.last = on;
        if !self.live {
            return 0.0;
        }
        let (tune, decay) = (p[1], p[2] * p[2]);
        let v = match voice {
            0 => self.kick(tune, decay),
            1 => self.snare(tune, decay),
            2 => self.clap(tune, decay),
            _ => self.hat(tune, decay),
        };
        self.t += DT;
        self.live = self.t < 4.0;
        v * p[3]
    }

    fn kick(&mut self, tune: f32, decay: f32) -> f32 {
        let t = self.t;
        let base = 40.0 * (tune * 1.25).exp2() * self.ratio;
        let hz = base * (1.0 + 2.5 * (-t / 0.035).exp() + 6.0 * (-t / 0.003).exp());
        self.phase[0] = (self.phase[0] + hz * DT).fract();
        let body = (self.phase[0] * TAU).sin() * (-t / (0.08 + decay * 0.7)).exp();
        // The beater: a millisecond of noise.
        let click = self.rng.next() * (-t / 0.0012).exp() * 0.3;
        (2.0 * (body + click)).tanh() * 0.8
    }

    fn snare(&mut self, tune: f32, decay: f32) -> f32 {
        let t = self.t;
        let hz = 160.0 * tune.exp2() * self.ratio * (1.0 + 0.5 * (-t / 0.015).exp());
        self.phase[0] = (self.phase[0] + hz * DT).fract();
        self.phase[1] = (self.phase[1] + hz * 1.71 * DT).fract();
        let tone =
            ((self.phase[0] * TAU).sin() + 0.6 * (self.phase[1] * TAU).sin()) * (-t / 0.045).exp();
        // The wires: a wide band of noise around 4.5 kHz.
        let [_, band, _] = self.svf.run(self.rng.next(), cutoff(4500.0), 1.6);
        let noise = band * (-t / (0.05 + decay * 0.3)).exp();
        tone * 0.45 + noise * 1.1
    }

    fn clap(&mut self, tune: f32, decay: f32) -> f32 {
        let t = self.t;
        let [_, band, _] = self.svf.run(
            self.rng.next(),
            cutoff(900.0 * (tune * 1.5).exp2() * self.ratio),
            0.6,
        );
        // Three quick slaps, then the room.
        let slap = |at: f32| {
            if t >= at {
                (-(t - at) / 0.0045).exp()
            } else {
                0.0
            }
        };
        let tail = if t >= 0.024 {
            0.7 * (-(t - 0.024) / (0.05 + decay * 0.4)).exp()
        } else {
            0.0
        };
        band * slap(0.0).max(slap(0.011)).max(slap(0.022)).max(tail) * 2.0
    }

    fn hat(&mut self, tune: f32, decay: f32) -> f32 {
        let t = self.t;
        let pitch = (tune - 0.5).exp2() * self.ratio;
        let mut metal = 0.0;
        for (phase, hz) in self.phase.iter_mut().zip(METAL) {
            *phase = (*phase + hz * pitch * DT).fract();
            metal += if *phase < 0.5 { 1.0 } else { -1.0 };
        }
        let x = metal / 6.0 * 0.7 + self.rng.next() * 0.3;
        let [_, band, _] = self.svf.run(x, cutoff(9000.0 * pitch), 0.8);
        band * (-t / (0.012 + decay * 0.35)).exp() * 1.4
    }
}
