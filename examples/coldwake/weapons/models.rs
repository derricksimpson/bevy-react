//! The three weapons, built from code in one frame: the muzzle points −Z,
//! the pistol grip and the foregrip sit where the engineer's pose puts his
//! hands (`engineer::pose::{GRIP, FOREGRIP}`), so the same model serves his
//! hands, the store's turntable and the inventory icons.
//!
//!   * Arc Cutter — a squat industrial tool, hazard yellow, three emitters
//!     side by side in a rotating head.
//!   * Pulse Driver — a gunmetal rifle, a drum underneath, three barrels in
//!     a shroud, orange vents.
//!   * Rail Spike — two rails and a glowing tungsten spike between them,
//!     coil rings along the way.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::station::kit::Build;

/// What a model's code needs to know about its own geometry.
pub struct Spec {
    /// The shot's origin.
    pub muzzle: Vec3,
    /// Where the tri-laser leaves (and so its pattern).
    pub lasers: &'static [Vec3],
    /// Where the ammo hologram floats.
    pub ammo: Vec3,
}

pub const CUTTER: Spec = Spec {
    muzzle: Vec3::new(0.0, 0.0, -0.32),
    lasers: &[
        Vec3::new(-0.026, 0.0, -0.315),
        Vec3::new(0.0, 0.0, -0.315),
        Vec3::new(0.026, 0.0, -0.315),
    ],
    ammo: Vec3::new(0.08, 0.115, -0.04),
};

pub const PULSE: Spec = Spec {
    muzzle: Vec3::new(0.0, 0.0, -0.5),
    lasers: &[
        Vec3::new(0.0, 0.018, -0.49),
        Vec3::new(-0.016, -0.01, -0.49),
        Vec3::new(0.016, -0.01, -0.49),
    ],
    ammo: Vec3::new(0.08, 0.115, -0.02),
};

pub const RAIL: Spec = Spec {
    muzzle: Vec3::new(0.0, 0.0, -0.5),
    lasers: &[Vec3::new(0.0, 0.03, -0.49), Vec3::new(0.0, -0.03, -0.49)],
    ammo: Vec3::new(0.08, 0.115, -0.02),
};

pub fn spec(weapon: &str) -> &'static Spec {
    match weapon {
        "pulse" => &PULSE,
        "rail" => &RAIL,
        _ => &CUTTER,
    }
}

/// Spawn `weapon`'s model (`cutter`, `pulse`, `rail`) at the origin, about
/// 0.6 m long with the muzzle toward −Z, every part on `layers`.
pub fn spawn_weapon_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    weapon: &str,
    layers: RenderLayers,
) -> Entity {
    let mut solid = |hex: &str, metal: f32, rough: f32| {
        materials.add(StandardMaterial {
            base_color: Srgba::hex(hex).unwrap().into(),
            metallic: metal,
            perceptual_roughness: rough,
            ..default()
        })
    };
    let m = Mats {
        paint: solid("b9821e", 0.35, 0.45),
        metal: solid("3a4046", 0.85, 0.35),
        dark: solid("17191b", 0.6, 0.5),
        rubber: solid("0b0c0d", 0.0, 0.85),
        bright: solid("8c949a", 0.95, 0.25),
        cyan: glow(materials, LinearRgba::rgb(0.6, 3.2, 4.6)),
        orange: glow(materials, LinearRgba::rgb(5.0, 1.6, 0.25)),
    };
    let mut b = Build::<StandardMaterial>::default();
    match weapon {
        "pulse" => pulse(&mut b, &m),
        "rail" => rail(&mut b, &m),
        _ => cutter(&mut b, &m),
    }
    let root = commands
        .spawn((Transform::default(), Visibility::default(), layers.clone()))
        .id();
    b.spawn(commands, meshes, Some(root), layers);
    root
}

struct Mats {
    paint: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    rubber: Handle<StandardMaterial>,
    bright: Handle<StandardMaterial>,
    cyan: Handle<StandardMaterial>,
    orange: Handle<StandardMaterial>,
}

fn glow(materials: &mut Assets<StandardMaterial>, color: LinearRgba) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: color,
        ..default()
    })
}

/// A cylinder along Z.
fn tube(b: &mut Build<StandardMaterial>, m: &Handle<StandardMaterial>, r: f32, len: f32, at: Vec3) {
    let mesh = Cylinder::new(r, len).mesh().resolution(14);
    b.add(
        m,
        mesh,
        Transform::from_translation(at)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    );
}

/// The pistol grip under the back and the foregrip under the front, where
/// the hands go.
fn grips(b: &mut Build<StandardMaterial>, m: &Mats, front: f32) {
    b.add(
        &m.rubber,
        Cuboid::new(0.036, 0.12, 0.048),
        Transform::from_xyz(0.0, -0.1, 0.07).with_rotation(Quat::from_rotation_x(0.28)),
    );
    b.cube(
        &m.dark,
        Vec3::new(0.03, 0.035, 0.07),
        Vec3::new(0.0, -0.06, 0.03),
    );
    b.cube(
        &m.rubber,
        Vec3::new(0.032, 0.085, 0.034),
        Vec3::new(0.0, -0.085, front),
    );
}

fn cutter(b: &mut Build<StandardMaterial>, m: &Mats) {
    // The body: a yellow shell over a dark frame, black hazard bands.
    b.cube(
        &m.paint,
        Vec3::new(0.078, 0.1, 0.26),
        Vec3::new(0.0, 0.0, -0.06),
    );
    b.cube(
        &m.dark,
        Vec3::new(0.084, 0.03, 0.27),
        Vec3::new(0.0, -0.045, -0.06),
    );
    for z in [-0.14, -0.1, 0.0] {
        b.cube(
            &m.dark,
            Vec3::new(0.082, 0.102, 0.018),
            Vec3::new(0.0, 0.0, z),
        );
    }
    // The battery under the back, its charge window, the cells' glow.
    b.cube(
        &m.metal,
        Vec3::new(0.07, 0.07, 0.1),
        Vec3::new(0.0, -0.075, 0.02),
    );
    for side in [-1.0, 1.0] {
        b.cube(
            &m.cyan,
            Vec3::new(0.004, 0.022, 0.07),
            Vec3::new(side * 0.041, 0.012, -0.05),
        );
    }
    b.cube(
        &m.dark,
        Vec3::new(0.022, 0.016, 0.2),
        Vec3::new(0.0, 0.058, -0.06),
    );
    // The head: a drum turned along the barrel, three emitters in a row.
    tube(b, &m.metal, 0.048, 0.09, Vec3::new(0.0, 0.0, -0.235));
    tube(b, &m.dark, 0.052, 0.012, Vec3::new(0.0, 0.0, -0.195));
    for x in [-0.026, 0.0, 0.026] {
        tube(b, &m.bright, 0.009, 0.04, Vec3::new(x, 0.0, -0.29));
        b.add(
            &m.cyan,
            Sphere::new(0.007).mesh().uv(8, 6),
            Transform::from_xyz(x, 0.0, -0.312),
        );
    }
    grips(b, m, -0.15);
}

fn pulse(b: &mut Build<StandardMaterial>, m: &Mats) {
    b.cube(
        &m.metal,
        Vec3::new(0.062, 0.09, 0.42),
        Vec3::new(0.0, 0.0, -0.08),
    );
    b.cube(
        &m.dark,
        Vec3::new(0.066, 0.025, 0.4),
        Vec3::new(0.0, 0.035, -0.08),
    );
    for side in [-1.0, 1.0] {
        for z in [-0.2, -0.16, -0.12] {
            b.cube(
                &m.orange,
                Vec3::new(0.004, 0.012, 0.022),
                Vec3::new(side * 0.033, 0.005, z),
            );
        }
    }
    // The shroud and its three barrels.
    tube(b, &m.dark, 0.034, 0.12, Vec3::new(0.0, 0.0, -0.33));
    for (x, y) in [(0.0, 0.018), (-0.016, -0.01), (0.016, -0.01)] {
        tube(b, &m.bright, 0.008, 0.2, Vec3::new(x, y, -0.39));
    }
    // The drum, the stock, a sight on top.
    b.add(
        &m.dark,
        Cylinder::new(0.06, 0.05).mesh().resolution(18),
        Transform::from_xyz(0.0, -0.085, -0.03)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
    );
    b.cube(
        &m.orange,
        Vec3::new(0.052, 0.006, 0.006),
        Vec3::new(0.0, -0.085, 0.032),
    );
    b.cube(
        &m.dark,
        Vec3::new(0.04, 0.07, 0.16),
        Vec3::new(0.0, -0.01, 0.2),
    );
    b.cube(
        &m.metal,
        Vec3::new(0.02, 0.03, 0.06),
        Vec3::new(0.0, 0.06, -0.04),
    );
    grips(b, m, -0.15);
}

fn rail(b: &mut Build<StandardMaterial>, m: &Mats) {
    // The breech block.
    b.cube(
        &m.metal,
        Vec3::new(0.072, 0.1, 0.2),
        Vec3::new(0.0, -0.005, 0.04),
    );
    b.cube(
        &m.dark,
        Vec3::new(0.076, 0.03, 0.2),
        Vec3::new(0.0, 0.04, 0.04),
    );
    // Two rails, coil rings along them, the spike between.
    for y in [0.03, -0.03] {
        b.cube(
            &m.dark,
            Vec3::new(0.05, 0.012, 0.52),
            Vec3::new(0.0, y, -0.24),
        );
    }
    for z in [-0.12, -0.26, -0.4] {
        b.add(
            &m.orange,
            Torus::new(0.034, 0.042)
                .mesh()
                .minor_resolution(6)
                .major_resolution(18),
            Transform::from_xyz(0.0, 0.0, z)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        );
    }
    tube(b, &m.bright, 0.007, 0.42, Vec3::new(0.0, 0.0, -0.24));
    b.add(
        &m.orange,
        Cone::new(0.012, 0.05).mesh().resolution(10),
        Transform::from_xyz(0.0, 0.0, -0.475)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    );
    b.cube(
        &m.paint,
        Vec3::new(0.074, 0.02, 0.06),
        Vec3::new(0.0, 0.055, 0.1),
    );
    grips(b, m, -0.13);
}
