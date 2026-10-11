//! Every cue React plays (`sound.cue`, see `ui/src/sound.ts`) by name, and
//! the interface's own, rendered sample by sample with the kit: holograms
//! chirp bright and electric, the menus tick. The suit's, the weapons' and
//! the fixtures' cues render in `gear.rs`.

use std::f32::consts::TAU;

use super::gear;
use super::kit::*;

/// Renders one cue: mono samples at `RATE`.
type Render = fn() -> Vec<f32>;

/// Every cue by name. `fire` is played as `fire-<weapon>` (the sound module
/// tracks the equipped weapon).
pub const SFX: [(&str, Render); 27] = [
    ("holoOpen", holo_open),
    ("holoClose", holo_close),
    ("tick", tick),
    ("select", select),
    ("deny", deny),
    ("tab", tab),
    ("buy", buy),
    ("install", install),
    ("hit", gear::hit),
    ("heal", gear::heal),
    ("stasis", gear::stasis),
    ("lowAir", gear::low_air),
    ("fire-cutter", gear::fire_cutter),
    ("fire-pulse", gear::fire_pulse),
    ("fire-rail", gear::fire_rail),
    ("dryFire", gear::dry_fire),
    ("reload", gear::reload),
    ("equip", gear::equip),
    ("unlock", gear::unlock),
    ("hackTick", gear::hack_tick),
    ("hackLock", gear::hack_lock),
    ("shock", gear::shock),
    ("save", save),
    ("elevator", gear::elevator),
    ("locator", locator),
    ("static", static_burst),
    ("log", log),
];

/// Noise shaped by an envelope and a lowpass (`hz`), added to `buf`.
pub(super) fn hiss(
    buf: &mut [f32],
    at: f32,
    len: f32,
    hz: f32,
    seed: u32,
    amp: impl Fn(f32) -> f32,
) {
    let mut n = noise(seed);
    let (a, mut y) = (coef(hz), 0.0);
    let start = (at * SR) as usize;
    for (i, s) in buf[start..]
        .iter_mut()
        .take((len * SR) as usize)
        .enumerate()
    {
        *s += lowpass(&mut y, n(), a) * amp(i as f32 / SR);
    }
}

/// Struck metal: inharmonic sine partials (`hz` × ratios) ringing down.
pub(super) fn metal(buf: &mut [f32], at: f32, len: f32, hz: f32, decay: f32, level: f32) {
    for (i, ratio) in [1.0, 1.59, 2.14, 2.87, 3.73].into_iter().enumerate() {
        let d = decay / (1.0 + i as f32 * 0.45);
        let g = level / (1.0 + i as f32 * 0.7);
        tone(
            buf,
            at,
            len,
            sine,
            move |_| hz * ratio,
            move |t| g * pluck(t, 0.0008, d),
        );
    }
}

/// A hologram unfolding: a bright electric glide up with a shimmer of
/// vibrato over a breath of high noise.
fn holo_open() -> Vec<f32> {
    let mut b = silence(0.42);
    let hz =
        |t: f32| (380.0 + 1500.0 * (t / 0.3).min(1.0).powf(0.6)) * (1.0 + 0.012 * (t * 80.0).sin());
    tone(&mut b, 0.0, 0.42, tri, hz, |t| pluck(t, 0.03, 0.16));
    tone(
        &mut b,
        0.02,
        0.38,
        sine,
        move |t| 2.01 * hz(t),
        |t| 0.35 * pluck(t, 0.04, 0.12),
    );
    hiss(&mut b, 0.0, 0.35, 7000.0, 11, |t| {
        0.25 * pluck(t, 0.08, 0.08)
    });
    normalize(&mut b, 0.2);
    b
}

/// Folding away: the glide falling, shorter.
fn holo_close() -> Vec<f32> {
    let mut b = silence(0.28);
    let hz = |t: f32| 1500.0 - 1150.0 * (t / 0.22).min(1.0).powf(0.7);
    tone(&mut b, 0.0, 0.28, tri, hz, |t| pluck(t, 0.005, 0.1));
    hiss(&mut b, 0.0, 0.2, 5000.0, 12, |t| {
        0.2 * pluck(t, 0.005, 0.05)
    });
    normalize(&mut b, 0.16);
    b
}

/// The smallest UI tick (hover).
fn tick() -> Vec<f32> {
    let mut b = silence(0.03);
    tone(
        &mut b,
        0.0,
        0.03,
        sine,
        |_| 2600.0,
        |t| pluck(t, 0.0005, 0.006),
    );
    normalize(&mut b, 0.09);
    b
}

/// A confirming blip up a fourth.
fn select() -> Vec<f32> {
    let mut b = silence(0.13);
    for (at, hz) in [(0.0, 1318.5), (0.045, 1760.0)] {
        tone(
            &mut b,
            at,
            0.08,
            tri,
            move |_| hz,
            |t| pluck(t, 0.001, 0.025),
        );
    }
    normalize(&mut b, 0.18);
    b
}

/// A refusal: a low dull double buzz.
fn deny() -> Vec<f32> {
    let mut b = silence(0.28);
    for at in [0.0, 0.13] {
        let amp = |t: f32| pluck(t, 0.004, 0.12);
        tone(&mut b, at, 0.1, saw, |_| 130.0, amp);
        tone(&mut b, at, 0.1, saw, |_| 138.0, amp);
    }
    let (a, mut y1, mut y2) = (coef(1800.0), 0.0, 0.0);
    for s in &mut b {
        *s = lowpass(&mut y2, lowpass(&mut y1, *s, a), a);
    }
    normalize(&mut b, 0.24);
    b
}

/// Two soft ticks, a page turning.
fn tab() -> Vec<f32> {
    let mut b = silence(0.1);
    for (at, hz) in [(0.0, 1975.5), (0.05, 2637.0)] {
        tone(
            &mut b,
            at,
            0.04,
            sine,
            move |_| hz,
            |t| pluck(t, 0.0005, 0.008),
        );
    }
    normalize(&mut b, 0.14);
    b
}

/// A purchase: three quick rising tones and a coin's ring.
fn buy() -> Vec<f32> {
    let mut b = silence(0.45);
    for (i, hz) in [1046.5, 1318.5, 1568.0].into_iter().enumerate() {
        tone(
            &mut b,
            i as f32 * 0.05,
            0.12,
            tri,
            move |_| hz,
            |t| pluck(t, 0.001, 0.04),
        );
    }
    metal(&mut b, 0.15, 0.3, 2100.0, 0.09, 0.4);
    normalize(&mut b, 0.22);
    b
}

/// Powering a circuit node: a whine climbing through a lowpass, then a snap.
fn install() -> Vec<f32> {
    let mut b = silence(0.62);
    let n = (0.5 * SR) as usize;
    let (mut phase, mut y) = (0.0, 0.0);
    for (i, s) in b[..n].iter_mut().enumerate() {
        let k = i as f32 / n as f32;
        phase = (phase + (180.0 + 1400.0 * k * k) / SR).fract();
        *s = lowpass(&mut y, saw(phase), coef(400.0 + 5000.0 * k)) * k;
    }
    metal(&mut b, 0.5, 0.12, 1700.0, 0.04, 0.8);
    normalize(&mut b, 0.22);
    b
}

/// The save station: a soft rising three-note chord.
fn save() -> Vec<f32> {
    let mut b = silence(0.9);
    for (i, hz) in [440.0, 554.37, 659.25].into_iter().enumerate() {
        tone(
            &mut b,
            i as f32 * 0.08,
            0.75,
            sine,
            move |_| hz,
            |t| pluck(t, 0.02, 0.28),
        );
        tone(
            &mut b,
            i as f32 * 0.08,
            0.5,
            sine,
            move |_| 2.0 * hz,
            |t| 0.2 * pluck(t, 0.02, 0.15),
        );
    }
    normalize(&mut b, 0.2);
    b
}

/// The locator: a sparkle rising along the line.
fn locator() -> Vec<f32> {
    let mut b = silence(0.75);
    let hz = |t: f32| 600.0 * 4f32.powf((t / 0.6).min(1.0));
    tone(&mut b, 0.0, 0.75, sine, hz, |t| {
        (0.5 + 0.5 * (t * 90.0).sin()) * pluck(t, 0.05, 0.25)
    });
    hiss(&mut b, 0.0, 0.6, 9000.0, 81, |t| 0.12 * pluck(t, 0.1, 0.2));
    normalize(&mut b, 0.2);
    b
}

/// The dip between fixtures: a swell of hologram static.
fn static_burst() -> Vec<f32> {
    let mut b = silence(0.55);
    let mut n = noise(91);
    let (mut lo, mut hi) = (0.0, 0.0);
    for (i, s) in b.iter_mut().enumerate() {
        let t = i as f32 / SR;
        let x = n();
        let band = lowpass(&mut lo, x, coef(5000.0)) - lowpass(&mut hi, x, coef(700.0));
        let env = (t / 0.15).min(1.0) * ((0.55 - t) / 0.3).clamp(0.0, 1.0);
        let crackle = if (i / 300) % 7 == 3 { 1.6 } else { 1.0 };
        *s = band * env * crackle;
    }
    normalize(&mut b, 0.2);
    b
}

/// An audio log: a garbled transmission — a voice-like buzz in syllables
/// through a wandering formant, under static, dropping out now and then.
fn log() -> Vec<f32> {
    const LEN: f32 = 3.6;
    let mut b = silence(LEN);
    let mut n = noise(101);
    let mut pick = noise(102);
    let (mut phase, mut f1, mut f2, mut air) = (0.0, 0.0, 0.0, 0.0);
    let mut formant = 700.0;
    let mut syllable = 0.0f32;
    for (i, s) in b.iter_mut().enumerate() {
        let t = i as f32 / SR;
        if i % 4410 == 0 {
            formant = 450.0 + 900.0 * (0.5 + 0.5 * pick());
            syllable = if pick() > -0.4 { 1.0 } else { 0.0 };
        }
        let pitch = 118.0 + 14.0 * (t * 2.3).sin() + 6.0 * (t * 7.1).sin();
        phase = (phase + pitch / SR).fract();
        let buzz = saw(phase);
        let body = lowpass(
            &mut f2,
            lowpass(&mut f1, buzz, coef(formant)),
            coef(formant * 2.2),
        );
        let gate = syllable * (0.5 + 0.5 * (TAU * 4.5 * t).sin()).powf(0.6);
        let hissy = lowpass(&mut air, n(), coef(6000.0));
        let dropout = if ((t * 3.0) as u32).is_multiple_of(5) {
            0.2
        } else {
            1.0
        };
        *s = (body * gate * 1.8 + 0.35 * hissy) * dropout;
    }
    normalize(&mut b, 0.24);
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every cue renders non-empty, finite and unclipped, and the React
    /// list names nothing that isn't here.
    #[test]
    fn cues_render_clean_and_match_react() {
        for &(name, render) in &SFX {
            let buf = render();
            assert!(!buf.is_empty(), "{name} is empty");
            assert!(
                buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "{name} clips or isn't finite"
            );
        }
        let ts = include_str!("../ui/src/sound.ts");
        let cues = ts
            .split("export type Cue =")
            .nth(1)
            .and_then(|r| r.split(';').next())
            .unwrap();
        for cue in cues.split('"').skip(1).step_by(2) {
            let known = cue == "fire" || SFX.iter().any(|(name, _)| *name == cue);
            assert!(known, "sound.ts names {cue:?}, which has no sound");
        }
    }
}
