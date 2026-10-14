//! The effects: a tape-ish delay and a plate reverb, both built on
//! [`Line`].

use std::f32::consts::{FRAC_PI_2, TAU};

use super::engine::{DT, Params, RATE};

/// A delay line: written a sample at a time, read any number of samples
/// back.
pub struct Line {
    buf: Vec<f32>,
    at: usize,
}

impl Line {
    /// Room for `len` samples back.
    pub fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; (len + 2).next_power_of_two()],
            at: 0,
        }
    }

    /// The sample written `n` samples ago (`1`: the last one).
    pub fn tap(&self, n: usize) -> f32 {
        self.buf[self.at.wrapping_sub(n) & (self.buf.len() - 1)]
    }

    /// Between samples, linearly.
    pub fn tap_frac(&self, n: f32) -> f32 {
        let (i, f) = (n as usize, n.fract());
        let a = self.tap(i);
        a + (self.tap(i + 1) - a) * f
    }

    pub fn push(&mut self, x: f32) {
        self.buf[self.at] = x;
        self.at = (self.at + 1) & (self.buf.len() - 1);
    }
}

/// The note values a synced delay steps through, in beats: a sixteenth to
/// a half, dotted ones between.
const NOTES: [f32; 6] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
/// The longest delay, seconds (a half note at 60 BPM).
const LONGEST: f32 = 2.0;

/// `time` as seconds: free (40 ms to a second), or a note value at `bpm`.
pub fn delay_secs(time: f32, sync: bool, bpm: f32) -> f32 {
    if sync {
        NOTES[((time * 5.0).round() as usize).min(5)] * 60.0 / bpm.max(1.0)
    } else {
        0.04 + time * 0.96
    }
}

pub struct Delay {
    line: Line,
    /// The delay, samples (gliding toward the knob's).
    time: f32,
    low: f32,
    high: f32,
}

impl Delay {
    pub fn new() -> Self {
        Self {
            line: Line::new((LONGEST * RATE as f32) as usize + 2),
            time: 0.0,
            low: 0.0,
            high: 0.0,
        }
    }

    /// `[time, feedback, mix, sync]`.
    pub fn step(&mut self, p: &Params, t: &Params, x: f32, bpm: f32) -> f32 {
        let want = delay_secs(p[0], t[3] >= 0.5, bpm).min(LONGEST) * RATE as f32;
        if self.time == 0.0 {
            self.time = want;
        }
        // The time glides like a tape's: a change bends the repeats.
        self.time += (want - self.time) * 0.0004;
        let echo = self.line.tap_frac(self.time.max(1.0));
        // Every repeat darker and thinner than the last, and saturating
        // softly, so even full feedback never runs away.
        self.low += (echo - self.low) * 0.3;
        self.high += (self.low - self.high) * 0.012;
        let wet = self.low - self.high;
        self.line.push(x + (wet * p[1] * 0.92).tanh());
        x + (wet - x) * p[2]
    }
}

/// A Schroeder allpass around a delay of `n` samples.
struct Allpass {
    line: Line,
    n: usize,
}

impl Allpass {
    fn new(n: usize) -> Self {
        Self {
            line: Line::new(n),
            n,
        }
    }

    fn step(&mut self, x: f32, g: f32) -> f32 {
        let d = self.line.tap(self.n);
        let w = x + g * d;
        self.line.push(w);
        d - g * w
    }
}

/// Dattorro's lengths are at 29761 Hz.
const fn at_rate(n: usize) -> usize {
    (n * RATE as usize + 14_880) / 29_761
}

/// The tank's allpasses swing this many samples either way.
const SWING: f32 = at_rate(16) as f32;
/// The output taps, per side: `(half, line, samples)` — line 0 the first
/// delay, 1 the second allpass, 2 the second delay — each added or
/// subtracted (Dattorro's table 2).
const TAPS: [[(usize, usize, usize, f32); 7]; 2] = [
    [
        (1, 0, at_rate(266), 1.0),
        (1, 0, at_rate(2974), 1.0),
        (1, 1, at_rate(1913), -1.0),
        (1, 2, at_rate(1996), 1.0),
        (0, 0, at_rate(1990), -1.0),
        (0, 1, at_rate(187), -1.0),
        (0, 2, at_rate(1066), -1.0),
    ],
    [
        (0, 0, at_rate(353), 1.0),
        (0, 0, at_rate(3627), 1.0),
        (0, 1, at_rate(1228), -1.0),
        (0, 2, at_rate(2673), 1.0),
        (1, 0, at_rate(2111), -1.0),
        (1, 1, at_rate(335), -1.0),
        (1, 2, at_rate(121), -1.0),
    ],
];
/// The wet signal's level against the dry.
const WET: f32 = 0.6;

/// One half of the tank: a swinging allpass, a delay, a damping lowpass, an
/// allpass and a delay.
struct Half {
    swung: Line,
    swung_len: f32,
    first: Line,
    first_len: usize,
    damp: f32,
    allpass: Allpass,
    last: Line,
    last_len: usize,
}

impl Half {
    fn new(swung: usize, first: usize, allpass: usize, last: usize) -> Self {
        let swung = at_rate(swung);
        Self {
            swung: Line::new(swung + SWING as usize + 2),
            swung_len: swung as f32,
            first: Line::new(at_rate(first)),
            first_len: at_rate(first),
            damp: 0.0,
            allpass: Allpass::new(at_rate(allpass)),
            last: Line::new(at_rate(last)),
            last_len: at_rate(last),
        }
    }

    /// What this half hands the other.
    fn end(&self) -> f32 {
        self.last.tap(self.last_len)
    }

    fn step(&mut self, x: f32, swing: f32, damping: f32, decay: f32, diffusion: f32) {
        // Dattorro's first tank allpass runs the other way round (-0.7).
        let d = self.swung.tap_frac(self.swung_len + swing);
        let w = x - 0.7 * d;
        self.swung.push(w);
        let v = self.first.tap(self.first_len);
        self.first.push(d + 0.7 * w);
        self.damp += (v - self.damp) * (1.0 - damping);
        let v = self.allpass.step(self.damp * decay, diffusion);
        self.last.push(v);
    }

    fn tap(&self, line: usize, n: usize) -> f32 {
        match line {
            0 => self.first.tap(n),
            1 => self.allpass.line.tap(n),
            _ => self.last.tap(n),
        }
    }
}

/// Dattorro's plate (1997): four allpasses smear the input, then a tank of
/// two halves feeds itself in a figure eight, losing `decay` and some
/// treble each way round; each side listens to taps spread over the tank.
pub struct Plate {
    pre: Line,
    /// The input band: the lowpass memories under the highpass and of the
    /// lowpass.
    under: f32,
    band: f32,
    diffusers: [Allpass; 4],
    tank: [Half; 2],
    /// The tank's slow swing, cycles.
    swing: f32,
}

impl Plate {
    pub fn new() -> Self {
        Self {
            pre: Line::new(RATE as usize / 20),
            under: 0.0,
            band: 0.0,
            diffusers: [142, 107, 379, 277].map(|n| Allpass::new(at_rate(n))),
            tank: [
                Half::new(672, 4453, 1800, 3720),
                Half::new(908, 4217, 2656, 3163),
            ],
            swing: 0.0,
        }
    }

    /// `[size, damp, mix]`: the bigger the plate, the longer its tail and
    /// the later it starts; damping darkens it.
    pub fn step(&mut self, p: &Params, x: f32) -> [f32; 2] {
        let decay = 0.25 + p[0] * 0.7;
        let damping = p[1] * 0.85;
        self.pre.push(x);
        let v = self.pre.tap_frac(1.0 + (0.004 + p[0] * 0.03) * RATE as f32);
        // 100 Hz to 9 kHz into the plate: no rumble, no fizz.
        self.under += (v - self.under) * 0.013;
        self.band += (v - self.under - self.band) * 0.69;
        let mut v = self.band;
        for (allpass, g) in self.diffusers.iter_mut().zip([0.75, 0.75, 0.625, 0.625]) {
            v = allpass.step(v, g);
        }
        self.swing = (self.swing + 0.8 * DT).fract();
        let ends = [self.tank[1].end(), self.tank[0].end()];
        let diffusion = (decay + 0.15).clamp(0.25, 0.5);
        for (k, (half, end)) in self.tank.iter_mut().zip(ends).enumerate() {
            let swing = (self.swing * TAU + k as f32 * FRAC_PI_2).sin() * SWING;
            half.step(v + end * decay, swing, damping, decay, diffusion);
        }
        let [left, right] = TAPS.map(|taps| {
            taps.iter()
                .map(|&(half, line, n, sign)| self.tank[half].tap(line, n) * sign)
                .sum::<f32>()
        });
        let (dry, wet) = ((p[2] * FRAC_PI_2).cos(), (p[2] * FRAC_PI_2).sin() * WET);
        [x * dry + left * wet, x * dry + right * wet]
    }
}
