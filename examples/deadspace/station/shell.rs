//! The room itself: grated floor, ribbed steel walls with the far wall cut
//! for the bulkhead and the elevator, conduits and pipes, the ceiling and
//! its two light strips. Static, merged per material (`Build`).

use bevy::prelude::*;

use super::kit::{Build, Kit, PaintMaterial, glow};
use crate::layout::{BULKHEAD, BULKHEAD_SIZE, ELEVATOR, ELEVATOR_SIZE, ROOM_MAX, ROOM_MIN};

/// Wall thickness behind the interior faces.
const WALL: f32 = 0.3;
/// Where the structural rings stand along the room (z); the rings run up
/// the side walls and across the ceiling.
const RIBS: [f32; 5] = [-3.75, -1.25, 1.25, 3.75, 6.1];
/// Rib z values cut short on a side wall (x sign) because a fixture stands
/// there: the kiosk and the bench (z −1.25), the title alcove (z 3.75).
const SHORT_RIBS: [(f32, f32); 3] = [(-1.0, -1.25), (1.0, -1.25), (-1.0, 3.75)];
/// The two ceiling light strips (z) and their diffuser segments (x).
const STRIPS: [f32; 2] = [-2.6, 3.2];
const SEGMENTS: [f32; 4] = [-6.75, -2.25, 2.25, 6.75];
const STRIP_Y: f32 = 4.18;

/// The diffusers that flicker: one segment per strip, each with its own
/// material (strip index, segment index).
const FLICKERING: [(usize, usize); 2] = [(0, 1), (1, 3)];

pub struct Shell {
    /// One material per flickering diffuser, in `FLICKERING` order.
    pub flicker: Vec<Handle<StandardMaterial>>,
}

pub fn build(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    kit: &Kit,
) -> Shell {
    let mut b = Build::<PaintMaterial>::default();
    let mut lit = Build::<StandardMaterial>::default();
    let (lo, hi) = (ROOM_MIN, ROOM_MAX);
    let width = hi.x - lo.x;
    let depth = hi.z - lo.z;
    let mid_z = (lo.z + hi.z) * 0.5;

    // Floor: one slab of grating, a gutter channel along each side wall,
    // and warning stripes before the bulkhead and the elevator.
    b.cube(
        &kit.floor,
        Vec3::new(width, 0.1, depth),
        Vec3::new(0.0, -0.05, mid_z),
    );
    for x in [lo.x + 0.3, hi.x - 0.3] {
        b.cube(
            &kit.dark,
            Vec3::new(0.6, 0.09, depth),
            Vec3::new(x, 0.045, mid_z),
        );
    }
    b.cube(
        &kit.dark,
        Vec3::new(width, 0.09, 0.45),
        Vec3::new(0.0, 0.045, lo.z + 0.22),
    );
    b.cube(
        &kit.hazard,
        Vec3::new(BULKHEAD_SIZE.x + 0.8, 0.012, 0.7),
        Vec3::new(BULKHEAD.x, 0.006, lo.z + 0.8),
    );
    b.cube(
        &kit.hazard,
        Vec3::new(ELEVATOR_SIZE.x + 0.6, 0.012, 0.6),
        Vec3::new(ELEVATOR.x, 0.006, lo.z + 0.75),
    );

    // Ceiling.
    b.cube(
        &kit.dark,
        Vec3::new(width, WALL, depth),
        Vec3::new(0.0, hi.y + WALL * 0.5, mid_z),
    );

    // Side walls and the near wall.
    for x in [lo.x - WALL * 0.5, hi.x + WALL * 0.5] {
        b.cube(
            &kit.steel,
            Vec3::new(WALL, hi.y, depth),
            Vec3::new(x, hi.y * 0.5, mid_z),
        );
    }
    b.cube(
        &kit.steel,
        Vec3::new(width, hi.y, WALL),
        Vec3::new(0.0, hi.y * 0.5, hi.z + WALL * 0.5),
    );

    // The far wall, around its two openings.
    far_wall(&mut b, kit);

    // A darker band low on every wall, and a conduit tray up high.
    for (x, sign) in [(lo.x, 1.0), (hi.x, -1.0)] {
        b.cube(
            &kit.dark,
            Vec3::new(0.06, 1.0, depth),
            Vec3::new(x + sign * 0.03, 0.5, mid_z),
        );
        b.cube(
            &kit.dark,
            Vec3::new(0.18, 0.16, depth),
            Vec3::new(x + sign * 0.09, 3.32, mid_z),
        );
    }

    // Structural rings: ribs up both side walls and a beam across the
    // ceiling; each rib a deep plate with a narrower face plate on it.
    for z in RIBS {
        for (x, sign) in [(lo.x, 1.0f32), (hi.x, -1.0)] {
            let short = SHORT_RIBS.iter().any(|&(s, rz)| s == -sign && rz == z);
            let (y0, y1) = if short { (2.62, hi.y) } else { (0.0, hi.y) };
            let h = y1 - y0;
            let c = (y0 + y1) * 0.5;
            b.cube(
                &kit.steel,
                Vec3::new(0.3, h, 0.42),
                Vec3::new(x + sign * 0.15, c, z),
            );
            b.cube(
                &kit.dark,
                Vec3::new(0.06, h, 0.24),
                Vec3::new(x + sign * 0.33, c, z),
            );
            // Feet and a bracket where the pipes cross.
            if !short {
                b.cube(
                    &kit.dark,
                    Vec3::new(0.42, 0.22, 0.5),
                    Vec3::new(x + sign * 0.21, 0.11, z),
                );
            }
            b.cube(
                &kit.dark,
                Vec3::new(0.5, 0.1, 0.12),
                Vec3::new(x + sign * 0.25, 3.82, z),
            );
        }
        b.cube(
            &kit.steel,
            Vec3::new(width, 0.34, 0.42),
            Vec3::new(0.0, hi.y - 0.17, z),
        );
        b.cube(
            &kit.dark,
            Vec3::new(width, 0.06, 0.24),
            Vec3::new(0.0, hi.y - 0.37, z),
        );
    }

    // Pipes: two along each side wall, one across the far wall over the
    // doors, two risers on the far wall.
    for (x, sign) in [(lo.x, 1.0), (hi.x, -1.0)] {
        lit.rod(
            &kit.pipe,
            0.075,
            Vec3::new(x + sign * 0.52, 3.82, lo.z),
            Vec3::new(x + sign * 0.52, 3.82, hi.z),
        );
        lit.rod(
            &kit.pipe,
            0.045,
            Vec3::new(x + sign * 0.4, 3.98, lo.z),
            Vec3::new(x + sign * 0.4, 3.98, hi.z),
        );
    }
    lit.rod(
        &kit.pipe,
        0.09,
        Vec3::new(lo.x, 3.9, lo.z + 0.28),
        Vec3::new(hi.x, 3.9, lo.z + 0.28),
    );
    for x in [-5.4, 6.3] {
        lit.rod(
            &kit.pipe,
            0.06,
            Vec3::new(x, 0.0, lo.z + 0.12),
            Vec3::new(x, hi.y, lo.z + 0.12),
        );
        for y in [0.6, 2.2, 3.6] {
            b.cube(
                &kit.dark,
                Vec3::new(0.2, 0.08, 0.2),
                Vec3::new(x, y, lo.z + 0.1),
            );
        }
    }

    // The light strips: a housing and four diffuser segments each.
    let mut flicker = Vec::new();
    for (si, z) in STRIPS.iter().enumerate() {
        b.cube(
            &kit.dark,
            Vec3::new(18.4, 0.16, 0.46),
            Vec3::new(0.0, STRIP_Y + 0.1, *z),
        );
        for (gi, x) in SEGMENTS.iter().enumerate() {
            let at = Vec3::new(*x, STRIP_Y, *z);
            if FLICKERING.contains(&(si, gi)) {
                let m = glow(materials, LinearRgba::rgb(3.2, 3.6, 3.8));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(4.0, 0.05, 0.3))),
                    MeshMaterial3d(m.clone()),
                    Transform::from_translation(at),
                    bevy::light::NotShadowCaster,
                ));
                flicker.push(m);
            } else {
                lit.cube(&kit.white, Vec3::new(4.0, 0.05, 0.3), at);
            }
        }
    }

    b.spawn(commands, meshes, None, ());
    lit.spawn(commands, meshes, None, bevy::light::NotShadowCaster);
    Shell { flicker }
}

/// The far wall: solid around the bulkhead's and the elevator's openings,
/// paneled, with a heavier plinth.
fn far_wall(b: &mut Build<PaintMaterial>, kit: &Kit) {
    let (lo, hi) = (ROOM_MIN, ROOM_MAX);
    let z = lo.z - WALL * 0.5;
    let bh = (
        BULKHEAD.x - BULKHEAD_SIZE.x * 0.5,
        BULKHEAD.x + BULKHEAD_SIZE.x * 0.5,
    );
    let el = (
        ELEVATOR.x - ELEVATOR_SIZE.x * 0.5,
        ELEVATOR.x + ELEVATOR_SIZE.x * 0.5,
    );
    // Full-height pieces between the openings.
    for (x0, x1) in [(lo.x, bh.0), (bh.1, el.0), (el.1, hi.x)] {
        b.cube(
            &kit.steel,
            Vec3::new(x1 - x0, hi.y, WALL),
            Vec3::new((x0 + x1) * 0.5, hi.y * 0.5, z),
        );
    }
    // Over the openings.
    for ((x0, x1), top) in [(bh, BULKHEAD_SIZE.y), (el, ELEVATOR_SIZE.y)] {
        let h = hi.y - top;
        b.cube(
            &kit.steel,
            Vec3::new(x1 - x0, h, WALL),
            Vec3::new((x0 + x1) * 0.5, top + h * 0.5, z),
        );
    }
    // Plinth and a band of darker panels at waist height, broken by the
    // openings.
    for (x0, x1) in [
        (lo.x, bh.0 - 0.4),
        (bh.1 + 0.4, el.0 - 0.3),
        (el.1 + 0.3, hi.x),
    ] {
        b.cube(
            &kit.dark,
            Vec3::new(x1 - x0, 0.9, 0.08),
            Vec3::new((x0 + x1) * 0.5, 0.45, lo.z + 0.04),
        );
        b.cube(
            &kit.dark,
            Vec3::new(x1 - x0, 0.12, 0.14),
            Vec3::new((x0 + x1) * 0.5, 2.55, lo.z + 0.07),
        );
    }
}
