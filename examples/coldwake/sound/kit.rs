//! The synthesis kit every sound is rendered with (the cyberpunk example's):
//! tones with pitch and envelope functions, one-pole filters, noise, a loop
//! fold. Mono or interleaved `f32` samples at [`RATE`].

use std::f32::consts::TAU;

/// Samples per second, every sound.
pub const RATE: u32 = 44_100;
pub(super) const SR: f32 = RATE as f32;

/// `secs` of silence to render into.
pub(super) fn silence(secs: f32) -> Vec<f32> {
    vec![0.0; (secs * SR) as usize]
}

/// Add a tone to `buf` from `at` for `len` seconds: `wave` is the shape of
/// one cycle (phase 0..1), `hz(t)` and `amp(t)` the pitch and envelope (`t`
/// in seconds since the tone began). The last 4 ms fade out, so a tone
/// never ends on a click.
pub(super) fn tone(
    buf: &mut [f32],
    at: f32,
    len: f32,
    wave: fn(f32) -> f32,
    hz: impl Fn(f32) -> f32,
    amp: impl Fn(f32) -> f32,
) {
    let mut phase = 0.0;
    let start = (at * SR) as usize;
    for (i, s) in buf[start..]
        .iter_mut()
        .take((len * SR) as usize)
        .enumerate()
    {
        let t = i as f32 / SR;
        *s += wave(phase) * amp(t) * ((len - t) / 0.004).min(1.0);
        phase = (phase + hz(t) / SR).fract();
    }
}

/// A struck envelope: a linear `attack`, an exponential decay.
pub(super) fn pluck(t: f32, attack: f32, decay: f32) -> f32 {
    (t / attack).min(1.0) * (-t / decay).exp()
}

/// 0 → 1 → 0 every `period` seconds (a raised cosine), `shift` periods late.
pub(super) fn lfo(t: f64, period: f64, shift: f64) -> f32 {
    (0.5 - 0.5 * (std::f64::consts::TAU * (t / period - shift)).cos()) as f32
}

pub(super) fn sine(p: f32) -> f32 {
    (TAU * p).sin()
}

pub(super) fn tri(p: f32) -> f32 {
    1.0 - 4.0 * (p - 0.5).abs()
}

pub(super) fn saw(p: f32) -> f32 {
    2.0 * p - 1.0
}

/// A sine pushed toward a square: rounder than one, edgier than a sine.
pub(super) fn soft_square(p: f32) -> f32 {
    (2.5 * sine(p)).tanh()
}

/// White noise in -1..1 (xorshift: the same every run).
pub(super) fn noise(mut seed: u32) -> impl FnMut() -> f32 {
    move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// One-pole lowpass coefficient for a cutoff in Hz.
pub(super) fn coef(hz: f32) -> f32 {
    1.0 - (-TAU * hz / SR).exp()
}

/// One step of a one-pole lowpass with state `y`.
pub(super) fn lowpass(y: &mut f32, x: f32, a: f32) -> f32 {
    *y += a * (x - *y);
    *y
}

/// Scale `buf` to peak at `peak`.
pub(super) fn normalize(buf: &mut [f32], peak: f32) {
    let max = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 0.0 {
        buf.iter_mut().for_each(|s| *s *= peak / max);
    }
}

/// Crossfade the last `frames` frames of a render over its first ones and
/// drop them: the render ran on past its loop point, so the seam is the
/// render's own next sample.
pub(super) fn fold(buf: &mut Vec<f32>, channels: usize, frames: usize) {
    let tail = buf.len() - frames * channels;
    for i in 0..frames * channels {
        let w = (i / channels) as f32 / frames as f32;
        buf[i] = buf[i] * w + buf[tail + i] * (1.0 - w);
    }
    buf.truncate(tail);
}
