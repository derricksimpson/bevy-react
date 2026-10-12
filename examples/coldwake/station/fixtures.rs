//! The fixtures the holograms hang on: the controls terminal's alcove, the
//! store kiosk, the upgrade bench, the save station, the hack panel, and the
//! housings of the elevator's call panel and floor indicator. Static,
//! merged per material. (The bulkhead and the elevator move: `doors.rs`.)
//!
//! Each fixture is built facing +Z in its own frame and turned onto its
//! wall, so "in front" is always +Z here.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

use super::kit::{Build, Kit, PaintMaterial};
use crate::layout::{
    BENCH, ELEVATOR, ELEVATOR_PANEL, HACK_PANEL, KIOSK, ROOM_MIN, SAVE_STATION, TERMINAL,
};

/// Two builders: wearing paint, and plain lit or glowing.
struct Parts {
    paint: Build<PaintMaterial>,
    lit: Build<StandardMaterial>,
    /// The fixture's frame: its wall point, turned to face the room.
    frame: Transform,
}

impl Parts {
    fn new(at: Vec3, facing: f32) -> Self {
        Self {
            paint: Build::default(),
            lit: Build::default(),
            frame: Transform::from_translation(at).with_rotation(Quat::from_rotation_y(facing)),
        }
    }

    fn place(&self, size: Vec3, at: Vec3) -> (Cuboid, Transform) {
        let t = self.frame * Transform::from_translation(at);
        (Cuboid::from_size(size), t)
    }

    fn cube(&mut self, m: &Handle<PaintMaterial>, size: Vec3, at: Vec3) {
        let (mesh, t) = self.place(size, at);
        self.paint.add(m, mesh, t);
    }

    fn lit_cube(&mut self, m: &Handle<StandardMaterial>, size: Vec3, at: Vec3) {
        let (mesh, t) = self.place(size, at);
        self.lit.add(m, mesh, t);
    }

    fn rod(&mut self, m: &Handle<StandardMaterial>, r: f32, a: Vec3, b: Vec3) {
        let (a, b) = (self.frame.transform_point(a), self.frame.transform_point(b));
        self.lit.rod(m, r, a, b);
    }

    fn spawn(self, commands: &mut Commands, meshes: &mut Assets<Mesh>) {
        self.paint.spawn(commands, meshes, None, ());
        self.lit.spawn(commands, meshes, None, ());
    }
}

/// Facing +X (the left wall), −X (the right wall), +Z (the far wall).
const ON_LEFT: f32 = FRAC_PI_2;
const ON_RIGHT: f32 = -FRAC_PI_2;
const ON_FAR: f32 = 0.0;

pub fn build(commands: &mut Commands, meshes: &mut Assets<Mesh>, kit: &Kit) {
    for parts in [
        terminal(kit),
        kiosk(kit),
        bench(kit),
        save_station(kit),
        hack_panel(kit),
        elevator_housings(kit),
    ] {
        parts.spawn(commands, meshes);
    }
}

/// The controls terminal: a deep frame around a dark screen recess (the
/// hologram floats in it), a console ledge below with its keys lit.
fn terminal(kit: &Kit) -> Parts {
    let mut p = Parts::new(TERMINAL, ON_LEFT);
    let (w, h, d, y) = (2.1, 1.3, 0.38, 1.6);
    p.lit_cube(&kit.screen, Vec3::new(w, h, 0.02), Vec3::new(0.0, y, 0.01));
    for side in [-1.0, 1.0] {
        p.cube(
            &kit.steel,
            Vec3::new(0.22, h + 0.44, d),
            Vec3::new(side * (w + 0.22) * 0.5, y, d * 0.5),
        );
        p.cube(
            &kit.dark,
            Vec3::new(0.05, h, 0.06),
            Vec3::new(side * (w * 0.5 - 0.02), y, d - 0.05),
        );
    }
    for (dy, hh) in [(h * 0.5 + 0.11, 0.22), (-h * 0.5 - 0.11, 0.22)] {
        p.cube(
            &kit.steel,
            Vec3::new(w + 0.44, hh, d),
            Vec3::new(0.0, y + dy, d * 0.5),
        );
    }
    p.lit_cube(
        &kit.cyan,
        Vec3::new(w - 0.1, 0.012, 0.02),
        Vec3::new(0.0, y - h * 0.5 + 0.01, d - 0.03),
    );
    p.lit_cube(
        &kit.amber,
        Vec3::new(0.08, 0.04, 0.02),
        Vec3::new(w * 0.5 - 0.1, y + h * 0.5 + 0.12, d + 0.005),
    );
    // The console: a ledge, its sloped key panel, the keys.
    p.cube(
        &kit.dark,
        Vec3::new(w + 0.2, 0.92, 0.45),
        Vec3::new(0.0, 0.46, 0.22),
    );
    p.cube(
        &kit.steel,
        Vec3::new(w + 0.3, 0.06, 0.55),
        Vec3::new(0.0, 0.95, 0.27),
    );
    for i in 0..9 {
        let x = -0.8 + i as f32 * 0.2;
        let m = if i % 4 == 1 {
            &kit.amber
        } else {
            &kit.cyan_dim
        };
        p.lit_cube(m, Vec3::new(0.12, 0.012, 0.06), Vec3::new(x, 0.985, 0.38));
    }
    p
}

/// The store kiosk: a cabinet between two lit pillars, a projector bar
/// overhead casting the hologram, a counter below it.
fn kiosk(kit: &Kit) -> Parts {
    let mut p = Parts::new(KIOSK, ON_LEFT);
    p.cube(
        &kit.dark,
        Vec3::new(2.8, 3.0, 0.16),
        Vec3::new(0.0, 1.5, 0.08),
    );
    for side in [-1.0, 1.0] {
        let x = side * 1.5;
        p.cube(
            &kit.steel,
            Vec3::new(0.32, 3.1, 0.72),
            Vec3::new(x, 1.55, 0.36),
        );
        p.cube(
            &kit.dark,
            Vec3::new(0.36, 0.3, 0.78),
            Vec3::new(x, 0.15, 0.39),
        );
        p.lit_cube(
            &kit.cyan,
            Vec3::new(0.025, 2.2, 0.025),
            Vec3::new(x - side * 0.17, 1.7, 0.62),
        );
    }
    // The projector bar and its emitter slot.
    p.cube(
        &kit.steel,
        Vec3::new(2.68, 0.3, 0.8),
        Vec3::new(0.0, 2.96, 0.4),
    );
    p.lit_cube(
        &kit.cyan,
        Vec3::new(2.3, 0.02, 0.05),
        Vec3::new(0.0, 2.8, 0.68),
    );
    // The counter.
    p.cube(
        &kit.dark,
        Vec3::new(2.62, 0.86, 0.6),
        Vec3::new(0.0, 0.43, 0.3),
    );
    p.cube(
        &kit.steel,
        Vec3::new(2.7, 0.07, 0.7),
        Vec3::new(0.0, 0.895, 0.35),
    );
    p.lit_cube(
        &kit.amber_dim,
        Vec3::new(2.4, 0.03, 0.01),
        Vec3::new(0.0, 0.7, 0.605),
    );
    p.lit_cube(
        &kit.cyan_dim,
        Vec3::new(2.5, 0.01, 0.04),
        Vec3::new(0.0, 0.935, 0.66),
    );
    p
}

/// The upgrade bench: a heavy table, a tool board on the wall, a vise and
/// a work lamp.
fn bench(kit: &Kit) -> Parts {
    let mut p = Parts::new(BENCH, ON_RIGHT);
    // The table: top, apron, legs, a lower shelf.
    p.cube(
        &kit.steel,
        Vec3::new(2.8, 0.08, 0.86),
        Vec3::new(0.0, 0.92, 0.43),
    );
    p.cube(
        &kit.hazard,
        Vec3::new(2.8, 0.1, 0.03),
        Vec3::new(0.0, 0.83, 0.85),
    );
    for x in [-1.3, 1.3] {
        for z in [0.1, 0.78] {
            p.cube(
                &kit.dark,
                Vec3::new(0.08, 0.88, 0.08),
                Vec3::new(x, 0.44, z),
            );
        }
    }
    p.cube(
        &kit.dark,
        Vec3::new(2.6, 0.05, 0.7),
        Vec3::new(0.0, 0.25, 0.43),
    );
    p.cube(
        &kit.rust,
        Vec3::new(0.5, 0.3, 0.4),
        Vec3::new(-0.7, 0.43, 0.42),
    );
    p.cube(
        &kit.rust,
        Vec3::new(0.36, 0.22, 0.36),
        Vec3::new(0.6, 0.39, 0.4),
    );
    // The tool board, and the tools on it.
    p.cube(
        &kit.dark,
        Vec3::new(2.7, 1.5, 0.05),
        Vec3::new(0.0, 1.85, 0.03),
    );
    let tools: [(f32, f32, f32, f32); 7] = [
        (-1.05, 1.95, 0.06, 0.55),
        (-0.9, 1.9, 0.05, 0.42),
        (-0.72, 2.0, 0.07, 0.6),
        (0.65, 1.85, 0.05, 0.5),
        (0.8, 1.95, 0.08, 0.65),
        (1.0, 1.9, 0.05, 0.38),
        (1.12, 2.02, 0.04, 0.5),
    ];
    for (x, y, w, h) in tools {
        p.cube(&kit.steel, Vec3::new(w, h, 0.03), Vec3::new(x, y, 0.07));
        p.cube(
            &kit.steel,
            Vec3::new(w * 2.2, 0.05, 0.03),
            Vec3::new(x, y + h * 0.5, 0.07),
        );
    }
    // A vise at the front corner.
    p.cube(
        &kit.rust,
        Vec3::new(0.24, 0.14, 0.2),
        Vec3::new(1.05, 1.03, 0.65),
    );
    p.cube(
        &kit.dark,
        Vec3::new(0.06, 0.14, 0.24),
        Vec3::new(0.92, 1.03, 0.65),
    );
    p.rod(
        &kit.pipe,
        0.012,
        Vec3::new(0.85, 1.03, 0.65),
        Vec3::new(1.25, 1.03, 0.65),
    );
    // The work lamp hangs on an arm out from the wall, above the
    // hologram's top edge as the camera sees it (anything lit behind the
    // hologram would show through it), right over its spot light.
    p.rod(
        &kit.pipe,
        0.02,
        Vec3::new(-0.05, 2.9, 0.03),
        Vec3::new(-0.05, 2.9, 1.2),
    );
    p.rod(
        &kit.pipe,
        0.016,
        Vec3::new(-0.05, 2.9, 1.2),
        Vec3::new(-0.05, 2.74, 1.2),
    );
    p.cube(
        &kit.dark,
        Vec3::new(0.2, 0.08, 0.16),
        Vec3::new(-0.05, 2.7, 1.2),
    );
    p.lit_cube(
        &kit.warm,
        Vec3::new(0.16, 0.01, 0.12),
        Vec3::new(-0.05, 2.655, 1.2),
    );
    p
}

/// The save station: a pillar against the wall with a lit core, a ring
/// projector on the floor in front of it.
fn save_station(kit: &Kit) -> Parts {
    let mut p = Parts::new(SAVE_STATION, ON_FAR);
    p.cube(
        &kit.steel,
        Vec3::new(0.74, 2.5, 0.3),
        Vec3::new(0.0, 1.25, 0.15),
    );
    p.cube(
        &kit.dark,
        Vec3::new(0.82, 0.28, 0.4),
        Vec3::new(0.0, 2.36, 0.2),
    );
    p.cube(
        &kit.dark,
        Vec3::new(0.34, 1.9, 0.04),
        Vec3::new(0.0, 1.15, 0.31),
    );
    // The light column stops under the screen (it would shine through
    // the hologram's glass).
    p.lit_cube(
        &kit.cyan,
        Vec3::new(0.06, 0.8, 0.02),
        Vec3::new(0.0, 0.7, 0.335),
    );
    p.lit_cube(
        &kit.cyan,
        Vec3::new(0.6, 0.02, 0.05),
        Vec3::new(0.0, 2.21, 0.39),
    );
    // The floor pad.
    let pad = Transform::from_translation(p.frame.transform_point(Vec3::new(0.0, 0.04, 0.48)));
    p.paint.add(&kit.dark, Cylinder::new(0.42, 0.08), pad);
    p.lit.add(
        &kit.cyan,
        Torus::new(0.33, 0.35),
        pad * Transform::from_xyz(0.0, 0.045, 0.0),
    );
    p
}

/// The hack panel: a wall box with its cover hanging open, the circuit
/// face lit, cables down to the floor.
fn hack_panel(kit: &Kit) -> Parts {
    let mut p = Parts::new(HACK_PANEL, ON_FAR);
    let y = 1.3;
    p.cube(
        &kit.dark,
        Vec3::new(0.52, 0.68, 0.14),
        Vec3::new(0.0, y, 0.07),
    );
    p.cube(
        &kit.steel,
        Vec3::new(0.46, 0.62, 0.02),
        Vec3::new(0.0, y, 0.145),
    );
    // Circuit traces and chips.
    for i in 0..6 {
        let yy = y - 0.24 + i as f32 * 0.095;
        let w = 0.12 + ((i * 37) % 5) as f32 * 0.05;
        p.lit_cube(
            &kit.cyan_dim,
            Vec3::new(w, 0.006, 0.004),
            Vec3::new(-0.08 + (i % 2) as f32 * 0.06, yy, 0.157),
        );
    }
    for (x, yy) in [(-0.12, y + 0.17), (0.1, y - 0.05), (0.12, y + 0.2)] {
        p.cube(
            &kit.dark,
            Vec3::new(0.08, 0.06, 0.012),
            Vec3::new(x, yy, 0.158),
        );
    }
    p.lit_cube(
        &kit.red,
        Vec3::new(0.025, 0.025, 0.01),
        Vec3::new(0.18, y + 0.27, 0.16),
    );
    p.lit_cube(
        &kit.amber,
        Vec3::new(0.025, 0.025, 0.01),
        Vec3::new(0.13, y + 0.27, 0.16),
    );
    // The cover, swung open on its left hinge.
    let hinge = Vec3::new(-0.26, y, 0.15);
    let cover = p.frame
        * Transform::from_translation(hinge)
        * Transform::from_rotation(Quat::from_rotation_y(-1.9))
        * Transform::from_xyz(0.25, 0.0, 0.0);
    p.paint
        .add(&kit.hazard, Cuboid::new(0.5, 0.66, 0.025), cover);
    // Cables.
    for (x, sag) in [(-0.12, 0.05), (0.02, -0.04), (0.14, 0.08)] {
        p.rod(
            &kit.rubber,
            0.012,
            Vec3::new(x, y - 0.33, 0.08),
            Vec3::new(x + sag, 0.02, 0.22),
        );
    }
    p
}

/// The housings behind the elevator's call panel and its floor indicator.
fn elevator_housings(kit: &Kit) -> Parts {
    let mut p = Parts::new(Vec3::new(0.0, 0.0, ROOM_MIN.z), ON_FAR);
    let panel = ELEVATOR_PANEL.x;
    p.cube(
        &kit.dark,
        Vec3::new(0.66, 0.96, 0.1),
        Vec3::new(panel, 1.45, 0.05),
    );
    p.cube(
        &kit.steel,
        Vec3::new(0.58, 0.86, 0.02),
        Vec3::new(panel, 1.45, 0.105),
    );
    p.lit_cube(
        &kit.cyan_dim,
        Vec3::new(0.5, 0.008, 0.01),
        Vec3::new(panel, 1.0, 0.115),
    );
    let ind = ELEVATOR.x;
    p.cube(
        &kit.dark,
        Vec3::new(1.06, 0.42, 0.1),
        Vec3::new(ind, 3.17, 0.05),
    );
    p.lit_cube(
        &kit.screen,
        Vec3::new(0.96, 0.32, 0.02),
        Vec3::new(ind, 3.17, 0.105),
    );
    p
}
