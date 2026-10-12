//! The cues of the suit, the weapons and the fixtures: thumps, rings of
//! plate, zaps and clunks (the interface's chirps and ticks are in
//! `sfx.rs`, which lists them all).

use std::f32::consts::{PI, TAU};

use super::kit::*;
use super::sfx::{hiss, metal};

/// A blow to the suit: a low thump, a crunch, a clang of plate.
pub(super) fn hit() -> Vec<f32> {
    let mut b = silence(0.5);
    tone(
        &mut b,
        0.0,
        0.3,
        sine,
        |t| 45.0 + 70.0 * (-t / 0.03).exp(),
        |t| pluck(t, 0.002, 0.09),
    );
    hiss(&mut b, 0.0, 0.12, 2500.0, 21, |t| {
        0.6 * pluck(t, 0.001, 0.03)
    });
    metal(&mut b, 0.005, 0.45, 520.0, 0.14, 0.35);
    normalize(&mut b, 0.32);
    b
}

/// A medpack: a soft rising shimmer.
pub(super) fn heal() -> Vec<f32> {
    let mut b = silence(0.7);
    for (i, hz) in [523.25, 659.25, 783.99, 1046.5].into_iter().enumerate() {
        let at = i as f32 * 0.07;
        tone(
            &mut b,
            at,
            0.55,
            sine,
            move |t| hz * (1.0 + 0.004 * (t * 30.0).sin()),
            |t| 0.6 * pluck(t, 0.03, 0.2),
        );
    }
    hiss(&mut b, 0.0, 0.6, 9000.0, 31, |t| 0.08 * pluck(t, 0.15, 0.2));
    normalize(&mut b, 0.2);
    b
}

/// The stasis module: a warbling sweep slowing as it falls, a soft thud.
pub(super) fn stasis() -> Vec<f32> {
    let mut b = silence(0.8);
    let hz = |t: f32| {
        (950.0 * (-t / 0.25).exp() + 140.0) * (1.0 + 0.08 * (TAU * (14.0 - 12.0 * t) * t).sin())
    };
    tone(&mut b, 0.0, 0.8, tri, hz, |t| pluck(t, 0.01, 0.3));
    tone(
        &mut b,
        0.0,
        0.3,
        sine,
        |t| 60.0 + 40.0 * (-t / 0.05).exp(),
        |t| 0.6 * pluck(t, 0.003, 0.1),
    );
    normalize(&mut b, 0.24);
    b
}

/// The air running low: two short beeps.
pub(super) fn low_air() -> Vec<f32> {
    let mut b = silence(0.32);
    for at in [0.0, 0.16] {
        tone(
            &mut b,
            at,
            0.1,
            soft_square,
            |_| 880.0,
            |t| pluck(t, 0.002, 0.08),
        );
    }
    normalize(&mut b, 0.16);
    b
}

/// The Arc Cutter: a plasma zap — a burst of noise and a falling buzz with a
/// sizzle on top.
pub(super) fn fire_cutter() -> Vec<f32> {
    let mut b = silence(0.4);
    tone(
        &mut b,
        0.0,
        0.3,
        saw,
        |t| 90.0 + 420.0 * (-t / 0.03).exp(),
        |t| pluck(t, 0.001, 0.08),
    );
    hiss(&mut b, 0.0, 0.35, 6000.0, 41, |t| {
        0.7 * pluck(t, 0.001, 0.06)
    });
    tone(
        &mut b,
        0.0,
        0.25,
        sine,
        |t| 3200.0 - 1500.0 * t,
        |t| 0.2 * pluck(t, 0.002, 0.07),
    );
    normalize(&mut b, 0.3);
    b
}

/// The Pulse Driver: a short hard crack.
pub(super) fn fire_pulse() -> Vec<f32> {
    let mut b = silence(0.16);
    tone(
        &mut b,
        0.0,
        0.12,
        sine,
        |t| 70.0 + 160.0 * (-t / 0.01).exp(),
        |t| pluck(t, 0.0005, 0.035),
    );
    hiss(&mut b, 0.0, 0.1, 4500.0, 42, |t| pluck(t, 0.0003, 0.018));
    normalize(&mut b, 0.28);
    b
}

/// The Rail Spike: a heavy thump, ringing steel and a rising whine.
pub(super) fn fire_rail() -> Vec<f32> {
    let mut b = silence(0.7);
    tone(
        &mut b,
        0.0,
        0.4,
        sine,
        |t| 38.0 + 90.0 * (-t / 0.04).exp(),
        |t| pluck(t, 0.001, 0.14),
    );
    metal(&mut b, 0.0, 0.65, 310.0, 0.22, 0.5);
    tone(
        &mut b,
        0.0,
        0.3,
        sine,
        |t| 1200.0 + 2400.0 * t,
        |t| 0.15 * pluck(t, 0.01, 0.1),
    );
    hiss(&mut b, 0.0, 0.15, 3000.0, 43, |t| {
        0.5 * pluck(t, 0.0005, 0.04)
    });
    normalize(&mut b, 0.32);
    b
}

/// An empty clip: a tiny dry click.
pub(super) fn dry_fire() -> Vec<f32> {
    let mut b = silence(0.05);
    metal(&mut b, 0.0, 0.05, 3100.0, 0.008, 1.0);
    normalize(&mut b, 0.16);
    b
}

/// Reloading: a slide of noise between two clacks.
pub(super) fn reload() -> Vec<f32> {
    let mut b = silence(0.5);
    metal(&mut b, 0.0, 0.08, 1400.0, 0.015, 1.0);
    hiss(&mut b, 0.06, 0.2, 3500.0, 51, |t| {
        0.4 * pluck(t, 0.05, 0.06)
    });
    metal(&mut b, 0.32, 0.12, 900.0, 0.03, 1.2);
    tone(
        &mut b,
        0.32,
        0.1,
        sine,
        |_| 80.0,
        |t| 0.5 * pluck(t, 0.001, 0.03),
    );
    normalize(&mut b, 0.24);
    b
}

/// Drawing another weapon: a swoosh and a click.
pub(super) fn equip() -> Vec<f32> {
    let mut b = silence(0.32);
    hiss(&mut b, 0.0, 0.22, 2200.0, 52, |t| {
        0.6 * (PI * t / 0.22).sin()
    });
    metal(&mut b, 0.2, 0.1, 1800.0, 0.02, 0.8);
    normalize(&mut b, 0.2);
    b
}

/// The bulkhead's lock giving: a heavy clunk, a rising chime, venting air.
pub(super) fn unlock() -> Vec<f32> {
    let mut b = silence(1.1);
    tone(
        &mut b,
        0.0,
        0.25,
        sine,
        |t| 50.0 + 50.0 * (-t / 0.03).exp(),
        |t| pluck(t, 0.002, 0.08),
    );
    metal(&mut b, 0.0, 0.4, 260.0, 0.12, 0.6);
    for (i, hz) in [659.25, 987.77].into_iter().enumerate() {
        tone(
            &mut b,
            0.12 + i as f32 * 0.09,
            0.4,
            tri,
            move |_| hz,
            |t| 0.5 * pluck(t, 0.002, 0.12),
        );
    }
    hiss(&mut b, 0.25, 0.85, 5000.0, 61, |t| {
        0.5 * (t / 0.1).min(1.0) * (-t / 0.35).exp()
    });
    normalize(&mut b, 0.28);
    b
}

/// The hack dial turning a notch.
pub(super) fn hack_tick() -> Vec<f32> {
    let mut b = silence(0.02);
    metal(&mut b, 0.0, 0.02, 2300.0, 0.004, 1.0);
    normalize(&mut b, 0.08);
    b
}

/// A tumbler locking: a clunk and a bright ding.
pub(super) fn hack_lock() -> Vec<f32> {
    let mut b = silence(0.5);
    tone(
        &mut b,
        0.0,
        0.12,
        sine,
        |t| 90.0 + 80.0 * (-t / 0.02).exp(),
        |t| pluck(t, 0.001, 0.04),
    );
    tone(
        &mut b,
        0.03,
        0.45,
        sine,
        |_| 1975.5,
        |t| 0.6 * pluck(t, 0.001, 0.14),
    );
    tone(
        &mut b,
        0.03,
        0.3,
        sine,
        |_| 2963.0,
        |t| 0.2 * pluck(t, 0.001, 0.08),
    );
    normalize(&mut b, 0.24);
    b
}

/// A shock from the panel: crackling bursts over a mains buzz.
pub(super) fn shock() -> Vec<f32> {
    let mut b = silence(0.55);
    tone(
        &mut b,
        0.0,
        0.5,
        saw,
        |_| 60.0,
        |t| 0.5 * pluck(t, 0.003, 0.2),
    );
    tone(
        &mut b,
        0.0,
        0.5,
        saw,
        |_| 120.0,
        |t| 0.3 * pluck(t, 0.003, 0.15),
    );
    let mut n = noise(71);
    let mut gate = noise(72);
    let mut open = 0.0f32;
    for (i, s) in b.iter_mut().enumerate() {
        if i % 220 == 0 {
            open = if gate() > 0.2 { 1.0 } else { 0.0 };
        }
        let t = i as f32 / SR;
        *s += n() * open * (-t / 0.18).exp();
    }
    normalize(&mut b, 0.3);
    b
}

/// The elevator's bell: two bright tones, struck.
pub(super) fn elevator() -> Vec<f32> {
    let mut b = silence(1.1);
    metal(&mut b, 0.0, 1.0, 880.0, 0.35, 0.6);
    metal(&mut b, 0.28, 0.8, 659.25, 0.3, 0.6);
    normalize(&mut b, 0.22);
    b
}
