//! The suit, built from code: a joint hierarchy (hips → spine → chest →
//! head, shoulders → elbows → wrists, thighs → knees → ankles), each joint
//! carrying its armor merged per material. Limbs hang along −Y from their
//! joint; the body faces −Z, so its back (+Z) is what the camera sees.
//!
//! The look is an engineering hard suit: a lean black under-suit carrying
//! angular plates laid over one another — layered pauldrons, plate caps on
//! the elbows and knees, a torso that tapers over a ribbed abdomen, a
//! helmet of overlapping plates with the ribbing running down its back and
//! sides. Rust-orange only on edges and stripes; cyan lights (the visor
//! slits face forward); a channel down the back for the spine hologram.

use std::f32::consts::FRAC_PI_4;

use bevy::prelude::*;

use super::limbs::{boot, forearm, hand, shin_armor, thigh_armor, upper_arm};
use crate::holo::HoloAnchor;
use crate::layout::{START, facing};
use crate::player::lock::{MENU_AT, MENU_FACING};
use crate::station::kit::{Build, Paint, PaintMaterial, Pattern};

/// The joints, filled in at spawn. On the suit's root, which the player
/// moves (`engineer::follow`).
#[derive(Component)]
pub struct Rig {
    pub hips: Entity,
    pub spine: Entity,
    pub chest: Entity,
    pub head: Entity,
    /// Shoulder, elbow, wrist — right then left.
    pub arms: [[Entity; 3]; 2],
    /// Thigh, knee, ankle — right then left.
    pub legs: [[Entity; 3]; 2],
    /// Where the weapon rides (a child of the root, placed by the pose).
    pub gun: Entity,
}

/// The suit's lights (visor slits, back lights, bracers): one material, so
/// a heal or a vacuum can tint them all.
#[derive(Resource)]
pub struct SuitLights(pub Handle<StandardMaterial>);

pub const SUIT_LIGHT: LinearRgba = LinearRgba::rgb(0.45, 2.4, 3.4);

/// Rest offsets of each joint from its parent.
pub const HIPS: Vec3 = Vec3::new(0.0, 0.98, 0.0);
pub const SPINE: Vec3 = Vec3::new(0.0, 0.08, 0.0);
pub const CHEST: Vec3 = Vec3::new(0.0, 0.22, 0.0);
pub const NECK: Vec3 = Vec3::new(0.0, 0.33, 0.0);
/// The right shoulder (the left mirrors x).
pub const SHOULDER: Vec3 = Vec3::new(0.205, 0.25, 0.0);
pub const UPPER_ARM: f32 = 0.29;
pub const FOREARM: f32 = 0.27;
/// The right hip (the left mirrors x), from the hips joint.
pub const HIP: Vec3 = Vec3::new(0.1, -0.05, 0.0);
pub const THIGH: f32 = 0.44;
pub const SHIN: f32 = 0.42;
/// How high the ankle joint stands off the floor.
pub const ANKLE_HEIGHT: f32 = 0.11;

pub(super) struct Mats {
    pub(super) plate: Handle<PaintMaterial>,
    /// Raised panels: a shade lighter, so stacked plates read as layers.
    pub(super) raised: Handle<PaintMaterial>,
    pub(super) trim: Handle<PaintMaterial>,
    pub(super) accent: Handle<PaintMaterial>,
    pub(super) under: Handle<StandardMaterial>,
    pub(super) light: Handle<StandardMaterial>,
}

/// One joint's armor, in two builders.
#[derive(Default)]
pub(super) struct Part {
    paint: Build<PaintMaterial>,
    pub(super) lit: Build<StandardMaterial>,
}

impl Part {
    /// A plate: a box turned by (pitch, yaw, roll).
    pub(super) fn plate(&mut self, m: &Handle<PaintMaterial>, size: Vec3, at: Vec3, rot: Vec3) {
        let t = Transform::from_translation(at).with_rotation(euler(rot));
        self.paint.add(m, Cuboid::from_size(size), t);
    }

    /// A plate with a narrower raised panel on its outer face (local ±Z,
    /// `out`): the stepped edge reads as hard-surface layering.
    pub(super) fn panel(&mut self, m: &Mats, size: Vec3, at: Vec3, rot: Vec3, out: f32) {
        self.plate(&m.plate, size, at, rot);
        let raised = Vec3::new(size.x * 0.74, size.y * 0.74, size.z * 0.6);
        let lift = euler(rot) * Vec3::Z * out * (size.z + raised.z) * 0.5;
        self.plate(&m.raised, raised, at + lift, rot);
    }

    /// Black under-suit: an ellipsoid of half-sizes `half` (fills the gaps
    /// between plates, so they read as seams).
    pub(super) fn body(&mut self, m: &Mats, half: Vec3, at: Vec3) {
        let t = Transform::from_translation(at).with_scale(half);
        self.lit
            .add(&m.under, Sphere::new(1.0).mesh().uv(16, 10), t);
    }

    /// Black under-suit: a capsule `len` long between its caps, along Y.
    pub(super) fn limb(&mut self, m: &Mats, r: f32, len: f32, at: Vec3) {
        let mesh = bevy::mesh::Capsule3dMeshBuilder::new(r, len, 14, 6);
        self.lit
            .add(&m.under, mesh, Transform::from_translation(at));
    }

    pub(super) fn light(&mut self, m: &Mats, size: Vec3, at: Vec3) {
        self.lit.cube(&m.light, size, at);
    }

    fn spawn(self, commands: &mut Commands, meshes: &mut Assets<Mesh>, joint: Entity) {
        self.paint.spawn(commands, meshes, Some(joint), ());
        self.lit.spawn(commands, meshes, Some(joint), ());
    }
}

fn euler(r: Vec3) -> Quat {
    Quat::from_euler(EulerRot::YXZ, r.y, r.x, r.z)
}

fn worn(color: &str, metal: f32, rough: f32, seed: f32) -> PaintMaterial {
    PaintMaterial {
        base: StandardMaterial {
            base_color: Srgba::hex(color).unwrap().into(),
            metallic: metal,
            perceptual_roughness: rough,
            ..default()
        },
        extension: Paint {
            mode: Vec4::new(Pattern::Worn as u8 as f32, seed, 1.0, 0.8),
            tint: LinearRgba::BLACK,
        },
    }
}

/// Spawn the engineer at `layout::START` and return the root.
pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    paints: &mut Assets<PaintMaterial>,
) -> Entity {
    let light = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: SUIT_LIGHT,
        ..default()
    });
    commands.insert_resource(SuitLights(light.clone()));
    let m = Mats {
        plate: paints.add(worn("383831", 0.72, 0.38, 3.0)),
        raised: paints.add(worn("4a4940", 0.7, 0.34, 5.0)),
        trim: paints.add(worn("1c1f22", 0.6, 0.5, 11.0)),
        accent: paints.add(worn("8a4a1e", 0.5, 0.5, 7.0)),
        under: materials.add(StandardMaterial {
            base_color: Srgba::hex("101214").unwrap().into(),
            perceptual_roughness: 0.75,
            metallic: 0.1,
            ..default()
        }),
        light,
    };

    let root = commands
        .spawn((Transform::from_translation(START), Visibility::default()))
        .id();
    let joint = |commands: &mut Commands, parent: Entity, at: Vec3| {
        commands
            .spawn((
                Transform::from_translation(at),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id()
    };

    let hips = joint(commands, root, HIPS);
    pelvis(&m).spawn(commands, meshes, hips);
    let spine = joint(commands, hips, SPINE);
    abdomen(&m).spawn(commands, meshes, spine);
    let chest = joint(commands, spine, CHEST);
    torso(&m).spawn(commands, meshes, chest);
    let head = joint(commands, chest, NECK);
    helmet(&m).spawn(commands, meshes, head);

    // The worn holograms: the spine down the back channel, stasis on the
    // right shoulder blade, air over the left shoulder. Each faces +Z
    // (backward, toward the camera).
    for (name, at) in [
        ("spine", Vec3::new(0.0, 0.03, 0.2)),
        ("stasis", Vec3::new(0.15, 0.15, 0.2)),
        ("air", Vec3::new(-0.25, 0.48, 0.16)),
    ] {
        commands.spawn((
            HoloAnchor(name),
            Transform::from_translation(at),
            Visibility::default(),
            ChildOf(chest),
        ));
    }
    // What the suit projects into the air around him rides the root, not
    // the body, so it doesn't sway with his breath: the menu ahead and to
    // his right (the camera locks on it); the prompt line beside his right
    // arm.
    for (name, at, toward) in [
        ("menu", MENU_AT, MENU_FACING),
        (
            "prompt",
            Vec3::new(0.66, 1.34, -0.05),
            Vec3::new(0.0, 0.1, 1.0),
        ),
    ] {
        commands.spawn((
            HoloAnchor(name),
            Transform::from_translation(at).with_rotation(facing(toward)),
            Visibility::default(),
            ChildOf(root),
        ));
    }

    let mut arms = [[Entity::PLACEHOLDER; 3]; 2];
    let mut legs = [[Entity::PLACEHOLDER; 3]; 2];
    for (i, side) in [1.0f32, -1.0].into_iter().enumerate() {
        let mirror = Vec3::new(side, 1.0, 1.0);
        let shoulder = joint(commands, chest, SHOULDER * mirror);
        upper_arm(&m, side).spawn(commands, meshes, shoulder);
        let elbow = joint(commands, shoulder, Vec3::new(0.0, -UPPER_ARM, 0.0));
        forearm(&m, side).spawn(commands, meshes, elbow);
        let wrist = joint(commands, elbow, Vec3::new(0.0, -FOREARM, 0.0));
        hand(&m).spawn(commands, meshes, wrist);
        arms[i] = [shoulder, elbow, wrist];

        let thigh = joint(commands, hips, HIP * mirror);
        thigh_armor(&m, side).spawn(commands, meshes, thigh);
        let knee = joint(commands, thigh, Vec3::new(0.0, -THIGH, 0.0));
        shin_armor(&m).spawn(commands, meshes, knee);
        let ankle = joint(commands, knee, Vec3::new(0.0, -SHIN, 0.0));
        boot(&m).spawn(commands, meshes, ankle);
        legs[i] = [thigh, knee, ankle];
    }
    let gun = joint(commands, root, Vec3::new(0.18, 1.0, -0.2));
    commands.entity(root).insert(Rig {
        hips,
        spine,
        chest,
        head,
        arms,
        legs,
        gun,
    });
    root
}

fn pelvis(m: &Mats) -> Part {
    let mut p = Part::default();
    p.body(m, Vec3::new(0.145, 0.09, 0.105), Vec3::new(0.0, -0.01, 0.0));
    // The belt: front, back, sides, a buckle.
    let belt = 0.04;
    p.plate(
        &m.trim,
        Vec3::new(0.24, 0.05, 0.02),
        Vec3::new(0.0, belt, -0.105),
        Vec3::ZERO,
    );
    p.plate(
        &m.trim,
        Vec3::new(0.22, 0.05, 0.02),
        Vec3::new(0.0, belt, 0.1),
        Vec3::ZERO,
    );
    p.plate(
        &m.accent,
        Vec3::new(0.045, 0.032, 0.01),
        Vec3::new(0.0, belt, -0.118),
        Vec3::ZERO,
    );
    for side in [-1.0, 1.0] {
        p.plate(
            &m.trim,
            Vec3::new(0.02, 0.05, 0.18),
            Vec3::new(side * 0.148, belt, 0.0),
            Vec3::ZERO,
        );
        // Front tassets, a hip plate stacked over each side, a back plate.
        p.panel(
            m,
            Vec3::new(0.1, 0.13, 0.016),
            Vec3::new(side * 0.062, -0.07, -0.108),
            Vec3::new(0.18, -side * 0.18, 0.0),
            -1.0,
        );
        p.plate(
            &m.plate,
            Vec3::new(0.018, 0.13, 0.13),
            Vec3::new(side * 0.158, -0.04, 0.0),
            Vec3::new(0.0, 0.0, -side * 0.14),
        );
        p.plate(
            &m.raised,
            Vec3::new(0.012, 0.09, 0.09),
            Vec3::new(side * 0.17, -0.035, 0.0),
            Vec3::new(0.0, 0.0, -side * 0.14),
        );
    }
    p.panel(
        m,
        Vec3::new(0.2, 0.11, 0.016),
        Vec3::new(0.0, -0.05, 0.1),
        Vec3::new(-0.2, 0.0, 0.0),
        1.0,
    );
    p
}

/// The abdomen: ribbed segments, narrowest at the waist, the back split
/// around the spine channel.
fn abdomen(m: &Mats) -> Part {
    let mut p = Part::default();
    p.body(m, Vec3::new(0.118, 0.13, 0.088), Vec3::new(0.0, 0.11, 0.0));
    for (i, y) in [0.025, 0.07, 0.115, 0.16, 0.205].into_iter().enumerate() {
        let w = 0.19 + i as f32 * 0.012;
        let mat = if i % 2 == 0 { &m.plate } else { &m.raised };
        p.plate(
            mat,
            Vec3::new(w, 0.034, 0.02),
            Vec3::new(0.0, y, -0.09),
            Vec3::new(0.1, 0.0, 0.0),
        );
        for side in [-1.0, 1.0] {
            p.plate(
                mat,
                Vec3::new(0.02, 0.034, 0.1),
                Vec3::new(side * (w * 0.5 + 0.004), y, -0.01),
                Vec3::ZERO,
            );
            p.plate(
                mat,
                Vec3::new(0.065, 0.034, 0.018),
                Vec3::new(side * 0.078, y, 0.082),
                Vec3::new(-0.1, 0.0, 0.0),
            );
        }
    }
    p.plate(
        &m.trim,
        Vec3::new(0.06, 0.22, 0.02),
        Vec3::new(0.0, 0.11, 0.088),
        Vec3::ZERO,
    );
    p
}

fn torso(m: &Mats) -> Part {
    let mut p = Part::default();
    p.body(m, Vec3::new(0.16, 0.165, 0.112), Vec3::new(0.0, 0.13, 0.0));
    // Front: pectoral plates angled off a sternum strip, a plate under
    // them, stepped side ribs.
    for side in [-1.0, 1.0] {
        p.panel(
            m,
            Vec3::new(0.15, 0.17, 0.024),
            Vec3::new(side * 0.078, 0.175, -0.102),
            Vec3::new(0.12, side * 0.3, 0.0),
            -1.0,
        );
        for (k, y) in [0.04, 0.095, 0.15].into_iter().enumerate() {
            p.plate(
                &m.plate,
                Vec3::new(0.024, 0.042, 0.15 - k as f32 * 0.02),
                Vec3::new(side * (0.152 + k as f32 * 0.008), y, -0.005),
                Vec3::new(0.0, 0.0, side * 0.12),
            );
        }
    }
    p.plate(
        &m.trim,
        Vec3::new(0.03, 0.2, 0.022),
        Vec3::new(0.0, 0.16, -0.12),
        Vec3::new(0.12, 0.0, 0.0),
    );
    p.panel(
        m,
        Vec3::new(0.2, 0.07, 0.022),
        Vec3::new(0.0, 0.035, -0.1),
        Vec3::new(-0.15, 0.0, 0.0),
        -1.0,
    );
    // Back: shoulder blades angled outward around the spine channel, the
    // channel and its rails, lower plates, two small lights.
    for side in [-1.0, 1.0] {
        p.panel(
            m,
            Vec3::new(0.13, 0.16, 0.02),
            Vec3::new(side * 0.1, 0.17, 0.104),
            Vec3::new(-0.12, side * 0.38, side * 0.1),
            1.0,
        );
        p.plate(
            &m.accent,
            Vec3::new(0.008, 0.13, 0.02),
            Vec3::new(side * 0.163, 0.16, 0.08),
            Vec3::new(-0.12, side * 0.38, side * 0.1),
        );
        p.panel(
            m,
            Vec3::new(0.1, 0.07, 0.018),
            Vec3::new(side * 0.088, -0.015, 0.103),
            Vec3::new(0.15, side * 0.3, 0.0),
            1.0,
        );
        p.plate(
            &m.raised,
            Vec3::new(0.02, 0.56, 0.034),
            Vec3::new(side * 0.05, 0.03, 0.146),
            Vec3::ZERO,
        );
        p.light(
            m,
            Vec3::new(0.007, 0.05, 0.007),
            Vec3::new(side * 0.155, -0.04, 0.1),
        );
    }
    p.plate(
        &m.trim,
        Vec3::new(0.075, 0.56, 0.03),
        Vec3::new(0.0, 0.03, 0.135),
        Vec3::ZERO,
    );
    // The collar: tall at the back, closing at the sides and front.
    p.panel(
        m,
        Vec3::new(0.18, 0.09, 0.02),
        Vec3::new(0.0, 0.33, 0.075),
        Vec3::new(-0.55, 0.0, 0.0),
        1.0,
    );
    for side in [-1.0, 1.0] {
        p.plate(
            &m.plate,
            Vec3::new(0.02, 0.07, 0.1),
            Vec3::new(side * 0.088, 0.33, 0.0),
            Vec3::new(0.0, 0.0, -side * 0.3),
        );
    }
    p.plate(
        &m.plate,
        Vec3::new(0.14, 0.04, 0.02),
        Vec3::new(0.0, 0.315, -0.082),
        Vec3::new(0.3, 0.0, 0.0),
    );
    p
}

/// The helmet: plates over a dark core — a crown, cheek plates, an angular
/// face carrying the visor slits, and rows of ribs shingled down the back
/// and around the sides.
fn helmet(m: &Mats) -> Part {
    let mut p = Part::default();
    p.limb(m, 0.05, 0.05, Vec3::new(0.0, 0.035, 0.0));
    let c = Vec3::new(0.0, 0.17, 0.01);
    let half = Vec3::new(0.108, 0.128, 0.132);
    p.body(m, half, c);
    // Ribs: rows down the back of the skull from the crown to the nape,
    // each a center plate and two wrapping round the sides.
    for (row, deg) in [64.0f32, 44.0, 24.0, 4.0, -16.0, -36.0]
        .into_iter()
        .enumerate()
    {
        let phi = deg.to_radians();
        let mat = if row % 2 == 0 { &m.plate } else { &m.raised };
        let lift = 0.012;
        for (i, az) in [
            -FRAC_PI_4 * 1.4,
            -FRAC_PI_4 * 0.7,
            0.0,
            FRAC_PI_4 * 0.7,
            FRAC_PI_4 * 1.4,
        ]
        .into_iter()
        .enumerate()
        {
            let n = Vec3::new(az.sin() * phi.cos(), phi.sin(), az.cos() * phi.cos());
            let at = c + n * (half + Vec3::splat(lift));
            let w = if i == 2 { 0.07 } else { 0.06 };
            p.plate(mat, Vec3::new(w, 0.03, 0.016), at, Vec3::new(-phi, az, 0.0));
        }
    }
    // The crown: a center plate and two angled off it.
    p.plate(
        &m.raised,
        Vec3::new(0.06, 0.016, 0.17),
        c + Vec3::new(0.0, 0.135, -0.01),
        Vec3::ZERO,
    );
    for side in [-1.0, 1.0] {
        p.plate(
            &m.plate,
            Vec3::new(0.06, 0.016, 0.15),
            c + Vec3::new(side * 0.06, 0.118, -0.01),
            Vec3::new(0.0, 0.0, -side * 0.5),
        );
        // Cheek plates, stepped.
        p.plate(
            &m.plate,
            Vec3::new(0.018, 0.11, 0.12),
            c + Vec3::new(side * 0.112, -0.025, -0.03),
            Vec3::new(0.0, side * 0.15, 0.0),
        );
        p.plate(
            &m.raised,
            Vec3::new(0.012, 0.075, 0.08),
            c + Vec3::new(side * 0.122, -0.025, -0.035),
            Vec3::new(0.0, side * 0.15, 0.0),
        );
    }
    // The face: a brow plate, the dark visor with its slits, the jaw.
    p.plate(
        &m.plate,
        Vec3::new(0.18, 0.04, 0.03),
        c + Vec3::new(0.0, 0.065, -0.122),
        Vec3::new(0.35, 0.0, 0.0),
    );
    p.plate(
        &m.trim,
        Vec3::new(0.17, 0.075, 0.03),
        c + Vec3::new(0.0, 0.005, -0.13),
        Vec3::new(-0.05, 0.0, 0.0),
    );
    for side in [-1.0, 1.0] {
        p.plate(
            &m.plate,
            Vec3::new(0.085, 0.06, 0.03),
            c + Vec3::new(side * 0.045, -0.075, -0.108),
            Vec3::new(0.3, -side * 0.35, 0.0),
        );
        p.light(
            m,
            Vec3::new(0.01, 0.04, 0.01),
            c + Vec3::new(side * 0.04, -0.005, -0.147),
        );
    }
    p.light(
        m,
        Vec3::new(0.13, 0.012, 0.01),
        c + Vec3::new(0.0, 0.022, -0.147),
    );
    p
}
