//! The suit's limbs (see `suit.rs`): layered pauldrons, plate caps over
//! the elbows and knees, bracers, shin guards, boots — slim under-suit
//! capsules with angular plates laid over them.

use bevy::prelude::*;

use super::suit::{Mats, Part};

/// The upper arm and its pauldron: three plates stacked down over the
/// shoulder, each turned further out, a rust stripe along the top edge.
pub(super) fn upper_arm(m: &Mats, side: f32) -> Part {
    let mut p = Part::default();
    for (k, (x, y, roll, w)) in [
        (0.03, 0.064, 0.42, 0.15),
        (0.062, 0.028, 0.82, 0.13),
        (0.078, -0.016, 1.2, 0.11),
    ]
    .into_iter()
    .enumerate()
    {
        let mat = if k == 1 { &m.raised } else { &m.plate };
        p.plate(
            mat,
            Vec3::new(0.115 - k as f32 * 0.012, 0.02, w + 0.03),
            Vec3::new(side * x, y, 0.0),
            Vec3::new(0.0, 0.0, -side * roll),
        );
    }
    p.plate(
        &m.accent,
        Vec3::new(0.012, 0.022, 0.17),
        Vec3::new(side * 0.083, 0.04, 0.0),
        Vec3::new(0.0, 0.0, -side * 0.42),
    );
    for z in [-0.075, 0.075] {
        p.plate(
            &m.trim,
            Vec3::new(0.09, 0.06, 0.012),
            Vec3::new(side * 0.05, 0.035, z),
            Vec3::new(0.0, 0.0, -side * 0.5),
        );
    }
    p.limb(m, 0.042, 0.2, Vec3::new(0.0, -0.15, 0.0));
    p.plate(
        &m.plate,
        Vec3::new(0.026, 0.15, 0.075),
        Vec3::new(side * 0.042, -0.15, 0.0),
        Vec3::ZERO,
    );
    p.plate(
        &m.raised,
        Vec3::new(0.016, 0.1, 0.05),
        Vec3::new(side * 0.058, -0.15, 0.0),
        Vec3::ZERO,
    );
    p.plate(
        &m.plate,
        Vec3::new(0.06, 0.12, 0.018),
        Vec3::new(0.0, -0.16, -0.04),
        Vec3::ZERO,
    );
    p
}

/// The forearm: a plate cap behind the elbow, a bracer of three plates, a
/// light on the outside.
pub(super) fn forearm(m: &Mats, side: f32) -> Part {
    let mut p = Part::default();
    p.plate(
        &m.plate,
        Vec3::new(0.07, 0.065, 0.024),
        Vec3::new(0.0, 0.0, 0.045),
        Vec3::new(-0.3, 0.0, 0.0),
    );
    p.plate(
        &m.raised,
        Vec3::new(0.05, 0.04, 0.018),
        Vec3::new(0.0, -0.03, 0.054),
        Vec3::new(-0.1, 0.0, 0.0),
    );
    p.limb(m, 0.038, 0.17, Vec3::new(0.0, -0.13, 0.0));
    p.plate(
        &m.plate,
        Vec3::new(0.024, 0.17, 0.072),
        Vec3::new(side * 0.04, -0.13, 0.0),
        Vec3::ZERO,
    );
    p.plate(
        &m.raised,
        Vec3::new(0.066, 0.15, 0.02),
        Vec3::new(0.0, -0.13, 0.04),
        Vec3::ZERO,
    );
    p.plate(
        &m.plate,
        Vec3::new(0.06, 0.13, 0.018),
        Vec3::new(0.0, -0.135, -0.038),
        Vec3::ZERO,
    );
    p.light(
        m,
        Vec3::new(0.006, 0.055, 0.012),
        Vec3::new(side * 0.053, -0.11, 0.0),
    );
    p
}

pub(super) fn hand(m: &Mats) -> Part {
    let mut p = Part::default();
    p.lit.cube(
        &m.under,
        Vec3::new(0.062, 0.075, 0.04),
        Vec3::new(0.0, -0.04, 0.0),
    );
    p.lit.cube(
        &m.under,
        Vec3::new(0.058, 0.055, 0.038),
        Vec3::new(0.0, -0.095, -0.01),
    );
    p.plate(
        &m.plate,
        Vec3::new(0.06, 0.045, 0.012),
        Vec3::new(0.0, -0.035, 0.024),
        Vec3::ZERO,
    );
    p
}

pub(super) fn thigh_armor(m: &Mats, side: f32) -> Part {
    let mut p = Part::default();
    p.limb(m, 0.064, 0.3, Vec3::new(0.0, -0.21, 0.0));
    p.plate(
        &m.trim,
        Vec3::new(0.12, 0.05, 0.13),
        Vec3::new(0.0, -0.03, 0.0),
        Vec3::ZERO,
    );
    p.panel(
        m,
        Vec3::new(0.11, 0.24, 0.022),
        Vec3::new(0.0, -0.19, -0.064),
        Vec3::new(-0.05, 0.0, 0.0),
        -1.0,
    );
    p.plate(
        &m.plate,
        Vec3::new(0.022, 0.22, 0.1),
        Vec3::new(side * 0.064, -0.17, 0.0),
        Vec3::ZERO,
    );
    p.plate(
        &m.accent,
        Vec3::new(0.006, 0.2, 0.012),
        Vec3::new(side * 0.076, -0.17, -0.036),
        Vec3::ZERO,
    );
    p.plate(
        &m.plate,
        Vec3::new(0.09, 0.16, 0.02),
        Vec3::new(0.0, -0.2, 0.062),
        Vec3::ZERO,
    );
    p
}

/// The shin: an angled cap over the knee, a ridged shin guard, calf and
/// side plates.
pub(super) fn shin_armor(m: &Mats) -> Part {
    let mut p = Part::default();
    p.panel(
        m,
        Vec3::new(0.085, 0.085, 0.024),
        Vec3::new(0.0, 0.005, -0.064),
        Vec3::new(0.15, 0.0, 0.0),
        -1.0,
    );
    p.plate(
        &m.plate,
        Vec3::new(0.075, 0.028, 0.06),
        Vec3::new(0.0, 0.048, -0.048),
        Vec3::new(0.5, 0.0, 0.0),
    );
    p.plate(
        &m.accent,
        Vec3::new(0.07, 0.008, 0.012),
        Vec3::new(0.0, -0.037, -0.082),
        Vec3::new(0.15, 0.0, 0.0),
    );
    p.limb(m, 0.052, 0.3, Vec3::new(0.0, -0.2, 0.0));
    p.panel(
        m,
        Vec3::new(0.085, 0.26, 0.022),
        Vec3::new(0.0, -0.205, -0.054),
        Vec3::ZERO,
        -1.0,
    );
    p.plate(
        &m.plate,
        Vec3::new(0.08, 0.18, 0.022),
        Vec3::new(0.0, -0.14, 0.05),
        Vec3::ZERO,
    );
    for side in [-1.0, 1.0] {
        p.plate(
            &m.plate,
            Vec3::new(0.018, 0.2, 0.07),
            Vec3::new(side * 0.052, -0.22, 0.0),
            Vec3::ZERO,
        );
    }
    p
}

pub(super) fn boot(m: &Mats) -> Part {
    let mut p = Part::default();
    p.lit.cube(
        &m.under,
        Vec3::new(0.1, 0.08, 0.22),
        Vec3::new(0.0, -0.05, -0.03),
    );
    p.plate(
        &m.trim,
        Vec3::new(0.11, 0.025, 0.27),
        Vec3::new(0.0, -0.098, -0.045),
        Vec3::ZERO,
    );
    p.plate(
        &m.plate,
        Vec3::new(0.1, 0.05, 0.08),
        Vec3::new(0.0, -0.065, -0.15),
        Vec3::new(0.35, 0.0, 0.0),
    );
    p.plate(
        &m.raised,
        Vec3::new(0.088, 0.02, 0.1),
        Vec3::new(0.0, -0.02, -0.08),
        Vec3::new(0.25, 0.0, 0.0),
    );
    p.plate(
        &m.plate,
        Vec3::new(0.095, 0.07, 0.05),
        Vec3::new(0.0, -0.055, 0.07),
        Vec3::ZERO,
    );
    p.plate(
        &m.trim,
        Vec3::new(0.11, 0.05, 0.1),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::ZERO,
    );
    p
}
