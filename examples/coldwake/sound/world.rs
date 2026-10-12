//! The world's own noises (`station::Noise`, written by the doors, the
//! weapons and the suit when something happens out in the room): heavy
//! machinery, impacts down the deck, his boots. React's cues cover the interface; these
//! cover the room.

use super::kit::*;

/// Renders one noise: mono samples at `RATE`.
type Render = fn() -> Vec<f32>;

/// Every world noise by its `Noise` name. `elevatorArrive` rings the
/// elevator's bell (the `elevator` cue).
pub const NOISES: [(&str, Render); 6] = [
    ("step", step),
    ("bulkheadOpen", bulkhead_open),
    ("elevatorDoors", elevator_doors),
    ("elevatorMove", elevator_move),
    ("impact", impact),
    ("stasisHit", stasis_hit),
];

/// Noise through a band (`lo`..`hi` Hz) with an envelope, added to `buf`.
fn band(buf: &mut [f32], at: f32, len: f32, lo: f32, hi: f32, seed: u32, amp: impl Fn(f32) -> f32) {
    let mut n = noise(seed);
    let (a_hi, a_lo, mut y_hi, mut y_lo) = (coef(hi), coef(lo), 0.0, 0.0);
    let start = (at * SR) as usize;
    for (i, s) in buf[start..]
        .iter_mut()
        .take((len * SR) as usize)
        .enumerate()
    {
        let x = n();
        *s += (lowpass(&mut y_hi, x, a_hi) - lowpass(&mut y_lo, x, a_lo)) * amp(i as f32 / SR);
    }
}

/// A boot on the deck's grating: a dull thud and the plate's short ring.
fn step() -> Vec<f32> {
    let mut b = silence(0.22);
    tone(
        &mut b,
        0.0,
        0.16,
        sine,
        |t| 62.0 + 50.0 * (-t / 0.015).exp(),
        |t| pluck(t, 0.002, 0.045),
    );
    band(&mut b, 0.0, 0.1, 300.0, 2600.0, 0x57E9, |t| {
        0.35 * pluck(t, 0.001, 0.02)
    });
    tone(
        &mut b,
        0.0,
        0.2,
        sine,
        |_| 410.0,
        |t| 0.08 * pluck(t, 0.001, 0.05),
    );
    normalize(&mut b, 0.13);
    b
}

/// The bulkhead giving way: a deep clunk, two halves grinding apart on a
/// rising servo, venting gas.
fn bulkhead_open() -> Vec<f32> {
    let mut b = silence(2.2);
    tone(
        &mut b,
        0.0,
        0.5,
        sine,
        |t| 34.0 + 40.0 * (-t / 0.04).exp(),
        |t| pluck(t, 0.003, 0.18),
    );
    tone(
        &mut b,
        0.15,
        1.6,
        saw,
        |t| 70.0 + 45.0 * (t / 1.6),
        |t| 0.18 * (t / 0.2).min(1.0) * ((1.6 - t) / 0.3).clamp(0.0, 1.0),
    );
    band(&mut b, 0.15, 1.6, 90.0, 700.0, 0x5EA1, |t| {
        0.5 * (t / 0.3).min(1.0) * ((1.6 - t) / 0.4).clamp(0.0, 1.0)
    });
    band(&mut b, 0.3, 1.8, 1500.0, 7000.0, 0x5EA2, |t| {
        0.35 * (t / 0.08).min(1.0) * (-t / 0.7).exp()
    });
    tone(
        &mut b,
        1.72,
        0.4,
        sine,
        |t| 40.0 + 25.0 * (-t / 0.03).exp(),
        |t| 0.7 * pluck(t, 0.003, 0.12),
    );
    let (a, mut y) = (coef(3000.0), 0.0);
    for s in &mut b {
        *s = lowpass(&mut y, *s, a);
    }
    normalize(&mut b, 0.32);
    b
}

/// Elevator doors: a servo whine and a thump where they meet.
fn elevator_doors() -> Vec<f32> {
    let mut b = silence(1.1);
    tone(
        &mut b,
        0.0,
        0.85,
        tri,
        |t| 210.0 + 60.0 * (std::f32::consts::PI * t / 0.85).sin(),
        |t| 0.3 * (t / 0.1).min(1.0) * ((0.85 - t) / 0.15).clamp(0.0, 1.0),
    );
    band(&mut b, 0.0, 0.85, 300.0, 2500.0, 0xE1E0, |t| {
        0.25 * (t / 0.1).min(1.0) * ((0.85 - t) / 0.15).clamp(0.0, 1.0)
    });
    tone(
        &mut b,
        0.85,
        0.25,
        sine,
        |t| 60.0 + 40.0 * (-t / 0.02).exp(),
        |t| 0.8 * pluck(t, 0.002, 0.06),
    );
    normalize(&mut b, 0.22);
    b
}

/// The car moving in its shaft: a low hum with a rattle, swelling and dying.
fn elevator_move() -> Vec<f32> {
    let mut b = silence(2.6);
    let swell = |t: f32| (t / 0.5).min(1.0) * ((2.6 - t) / 0.6).clamp(0.0, 1.0);
    tone(
        &mut b,
        0.0,
        2.6,
        saw,
        |t| 55.0 + 4.0 * (t * 3.0).sin(),
        move |t| 0.25 * swell(t),
    );
    tone(&mut b, 0.0, 2.6, sine, |_| 110.0, move |t| 0.3 * swell(t));
    band(&mut b, 0.0, 2.6, 400.0, 1800.0, 0xE1E1, move |t| {
        0.18 * swell(t) * (0.6 + 0.4 * (t * 37.0).sin())
    });
    let (a, mut y) = (coef(900.0), 0.0);
    for s in &mut b {
        *s = lowpass(&mut y, *s, a);
    }
    normalize(&mut b, 0.2);
    b
}

/// A bolt striking the far wall: a thud and a ring of plate.
fn impact() -> Vec<f32> {
    let mut b = silence(0.5);
    tone(
        &mut b,
        0.0,
        0.3,
        sine,
        |t| 50.0 + 60.0 * (-t / 0.02).exp(),
        |t| pluck(t, 0.001, 0.07),
    );
    band(&mut b, 0.0, 0.2, 800.0, 6000.0, 0x1A9A, |t| {
        0.6 * pluck(t, 0.0005, 0.03)
    });
    for (ratio, decay) in [(1.0, 0.12), (1.71, 0.08), (2.53, 0.05)] {
        tone(
            &mut b,
            0.0,
            0.45,
            sine,
            move |_| 690.0 * ratio,
            move |t| 0.25 * pluck(t, 0.001, decay),
        );
    }
    normalize(&mut b, 0.26);
    b
}

/// The stasis field landing: a hollow, slowing whomp.
fn stasis_hit() -> Vec<f32> {
    let mut b = silence(0.8);
    tone(
        &mut b,
        0.0,
        0.8,
        sine,
        |t| 90.0 * (-t / 0.4).exp() + 45.0,
        |t| pluck(t, 0.01, 0.25),
    );
    tone(
        &mut b,
        0.0,
        0.7,
        tri,
        |t| 420.0 * (-t / 0.3).exp() + 160.0,
        |t| 0.25 * pluck(t, 0.02, 0.2),
    );
    band(&mut b, 0.0, 0.6, 2000.0, 8000.0, 0x57A5, |t| {
        0.15 * pluck(t, 0.05, 0.15)
    });
    normalize(&mut b, 0.24);
    b
}

#[cfg(test)]
mod tests {
    use super::NOISES;

    /// Every noise renders non-empty, finite and unclipped.
    #[test]
    fn noises_render_clean() {
        for &(name, render) in &NOISES {
            let buf = render();
            assert!(!buf.is_empty(), "{name} is empty");
            assert!(
                buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "{name} clips or isn't finite"
            );
        }
    }
}
