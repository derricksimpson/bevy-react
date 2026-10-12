//! The loops under everything: the station's drone (stereo, 16 s) and the
//! suit's breathing in a vacuum (mono, 4 s). Every partial repeats a whole
//! number of times per loop and every swell returns to its start; the
//! drone's noise is rendered past the loop and folded over its start, so
//! the seams are silent.

use std::f32::consts::TAU;

use super::kit::*;

/// Seconds per drone loop.
const LOOP: f64 = 16.0;
/// Seconds rendered past the loop and folded over its start (`fold`).
const FOLD: f64 = 2.0;

/// A whole number of cycles per loop.
fn looped(hz: f64) -> f64 {
    (hz * LOOP).round() / LOOP
}

/// A two-pole resonator (a ringing bandpass) at `hz`, `q` sharp.
struct Resonator {
    y1: f32,
    y2: f32,
}

impl Resonator {
    fn step(&mut self, x: f32, hz: f32, r: f32) -> f32 {
        let y = (1.0 - r) * x + 2.0 * r * (TAU * hz / SR).cos() * self.y1 - r * r * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// The station breathing around you: a low rumble, two low tones beating
/// against each other, the hull groaning through two slow resonances, a
/// faint electrical whine, and twice per loop a distant knock, once on each
/// side.
pub fn drone() -> Vec<f32> {
    let frames = ((LOOP + FOLD) * RATE as f64) as usize;
    let osc = |hz: f64, t: f64| (hz * t).fract() as f32;
    let (e1, f1, whine) = (looped(41.2), looped(43.65), looped(3100.0));
    let mut rumble = noise(0xD5_0001);
    let mut grit = noise(0xD5_0002);
    let mut lows = [0.0f32; 2];
    let mut groans = [
        Resonator { y1: 0.0, y2: 0.0 },
        Resonator { y1: 0.0, y2: 0.0 },
    ];
    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let t = i as f64 / RATE as f64;
        let swell = lfo(t, LOOP, 0.0);
        let beat = 0.6 * sine(osc(e1, t)) + 0.6 * sine(osc(f1, t));
        let whine = 0.012 * sine(osc(whine, t)) * lfo(t, 4.0, 0.0);
        let g = grit();
        for (side, (low, groan)) in lows.iter_mut().zip(groans.iter_mut()).enumerate() {
            let d = side as f32 * 2.0 - 1.0;
            let rum = lowpass(low, rumble(), coef(110.0)) * (0.7 + 0.3 * swell);
            // The groan slides a little and comes and goes, each side half a
            // loop apart.
            let bend = 1.0 + 0.04 * lfo(t, LOOP / 2.0, side as f64 * 0.5);
            let hz = if side == 0 { 212.0 } else { 337.0 } * bend;
            let groan = groan.step(g, hz, 0.9985) * 0.08 * lfo(t, LOOP, side as f64 * 0.5).powi(2);
            out.push(1.6 * rum + 0.25 * beat * (0.8 + 0.2 * d) + groan + whine);
        }
    }
    // Two knocks, far away, one per side.
    for (at, side) in [(5.3f32, 0usize), (11.7, 1)] {
        let start = (at * SR) as usize;
        for k in 0..(0.6 * SR) as usize {
            let t = k as f32 / SR;
            let thud = (TAU * (48.0 + 30.0 * (-t / 0.02).exp()) * t).sin() * pluck(t, 0.003, 0.12);
            let ring = (TAU * 181.0 * t).sin() * 0.25 * pluck(t, 0.002, 0.2);
            out[(start + k) * 2 + side] += 0.5 * (thud + ring);
            out[(start + k) * 2 + 1 - side] += 0.2 * (thud + ring);
        }
    }
    fold(&mut out, 2, (FOLD * RATE as f64) as usize);
    normalize(&mut out, 0.2);
    out
}

/// Breathing inside a sealed helmet, with the heart under it: a 1.4 s
/// inhale, a held beat, a long exhale; four muffled heartbeats per loop.
pub fn breath() -> Vec<f32> {
    let mut b = silence(4.0);
    let mut n = noise(0xB2_EA7);
    let (mut lo, mut hi) = (0.0, 0.0);
    for (i, s) in b.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let (env, cut) = if t < 1.4 {
            ((std::f32::consts::PI * t / 1.4).sin().powf(1.5), 1100.0)
        } else if t < 1.7 {
            (0.0, 800.0)
        } else if t < 3.6 {
            let k = (t - 1.7) / 1.9;
            ((std::f32::consts::PI * k).sin() * (1.0 - 0.4 * k), 650.0)
        } else {
            (0.0, 600.0)
        };
        let x = n();
        let band = lowpass(&mut lo, x, coef(cut)) - lowpass(&mut hi, x, coef(180.0));
        *s = band * env;
    }
    for beat in 0..4 {
        for (offset, level) in [(0.0, 1.0), (0.28, 0.7)] {
            let at = beat as f32 + offset;
            tone(
                &mut b,
                at,
                0.2,
                sine,
                |t| 42.0 + 25.0 * (-t / 0.02).exp(),
                move |t| level * 0.5 * pluck(t, 0.004, 0.06),
            );
        }
    }
    normalize(&mut b, 0.22);
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both loops render finite and unclipped.
    #[test]
    fn loops_render_clean() {
        for (name, buf) in [("drone", drone()), ("breath", breath())] {
            assert!(!buf.is_empty(), "{name} is empty");
            assert!(
                buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "{name} clips or isn't finite"
            );
        }
    }
}
