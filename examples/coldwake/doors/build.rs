//! The far wall's machinery, built once at startup: the bulkhead's frame,
//! its two sliding halves and the corridor stub beyond, the elevator's frame,
//! doors and lit car.

use super::*;

pub(super) fn build(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    kit: Res<Kit>,
) {
    let lock = glow(&mut materials, LinearRgba::rgb(5.0, 0.3, 0.15));
    let car_light = glow(&mut materials, CAR_LIGHT);
    let mut b = Build::<PaintMaterial>::default();
    let mut lit = Build::<StandardMaterial>::default();
    bulkhead_frame(&mut b, &mut lit, &kit);
    corridor(&mut b, &mut lit, &kit);
    elevator_frame(&mut b, &mut lit, &kit);
    car(&mut b, &mut lit, &kit, &car_light);
    b.spawn(&mut commands, &mut meshes, None, ());
    lit.spawn(
        &mut commands,
        &mut meshes,
        None,
        bevy::light::NotShadowCaster,
    );

    for dir in [-1.0, 1.0] {
        let closed = BULKHEAD.x + dir * (BULKHEAD_SIZE.x * 0.25 + 0.01);
        let root = commands
            .spawn((
                Half {
                    closed,
                    dir,
                    elevator: false,
                },
                Transform::from_xyz(closed, 0.0, DOOR_Z),
                Visibility::default(),
            ))
            .id();
        bulkhead_half(&mut commands, &mut meshes, &kit, &lock, root, dir);
        let closed = ELEVATOR.x + dir * ELEVATOR_SIZE.x * 0.25;
        let root = commands
            .spawn((
                Half {
                    closed,
                    dir,
                    elevator: true,
                },
                Transform::from_xyz(closed, 0.0, DOOR_Z),
                Visibility::default(),
            ))
            .id();
        // Brushed panels, a lit seam where the halves meet, a window slot.
        let mut half = Build::<StandardMaterial>::default();
        let w = ELEVATOR_SIZE.x * 0.5;
        let h = ELEVATOR_SIZE.y;
        half.cube(
            &kit.pipe,
            Vec3::new(w, h, 0.08),
            Vec3::new(0.0, h * 0.5, 0.0),
        );
        for y in [0.55, 2.35] {
            half.cube(
                &kit.rubber,
                Vec3::new(w - 0.1, 0.04, 0.1),
                Vec3::new(0.0, y, 0.0),
            );
        }
        half.cube(
            &kit.screen,
            Vec3::new(0.12, 0.7, 0.09),
            Vec3::new(-dir * 0.22, 1.6, 0.0),
        );
        half.cube(
            &kit.amber_dim,
            Vec3::new(0.02, h - 0.3, 0.1),
            Vec3::new(-dir * (w * 0.5 - 0.012), h * 0.5, 0.0),
        );
        half.spawn(&mut commands, &mut meshes, Some(root), ());
    }

    commands.insert_resource(Bulkhead {
        since: None,
        opened: false,
        lock,
    });
    commands.insert_resource(Elevator {
        phase: Phase::Idle,
        doors: 0.0,
        floor: 2,
        car_light,
    });
}

/// The bulkhead's frame: heavy jambs and lintel striped where things
/// close, pistons up the jambs, a red lamp over it all.
fn bulkhead_frame(b: &mut Build<PaintMaterial>, lit: &mut Build<StandardMaterial>, kit: &Kit) {
    let (w, h) = (BULKHEAD_SIZE.x, BULKHEAD_SIZE.y);
    let z = ROOM_MIN.z;
    for side in [-1.0f32, 1.0] {
        let x = BULKHEAD.x + side * (w * 0.5 + 0.2);
        b.cube(
            &kit.steel,
            Vec3::new(0.4, h + 0.55, 0.42),
            Vec3::new(x, (h + 0.55) * 0.5, z + 0.15),
        );
        b.cube(
            &kit.hazard,
            Vec3::new(0.06, h, 0.36),
            Vec3::new(x - side * 0.2, h * 0.5, z + 0.15),
        );
        b.cube(
            &kit.dark,
            Vec3::new(0.16, h + 0.3, 0.1),
            Vec3::new(x + side * 0.08, (h + 0.3) * 0.5, z + 0.41),
        );
        lit.rod(
            &kit.pipe,
            0.045,
            Vec3::new(x + side * 0.1, 0.3, z + 0.48),
            Vec3::new(x + side * 0.1, 2.4, z + 0.48),
        );
        lit.rod(
            &kit.pipe,
            0.07,
            Vec3::new(x + side * 0.1, 2.3, z + 0.48),
            Vec3::new(x + side * 0.1, 3.3, z + 0.48),
        );
    }
    b.cube(
        &kit.steel,
        Vec3::new(w + 0.8, 0.55, 0.46),
        Vec3::new(BULKHEAD.x, h + 0.27, z + 0.17),
    );
    b.cube(
        &kit.hazard,
        Vec3::new(w, 0.06, 0.36),
        Vec3::new(BULKHEAD.x, h + 0.02, z + 0.15),
    );
    b.cube(
        &kit.dark,
        Vec3::new(0.5, 0.16, 0.12),
        Vec3::new(BULKHEAD.x, h + 0.38, z + 0.44),
    );
    lit.cube(
        &kit.red,
        Vec3::new(0.36, 0.05, 0.03),
        Vec3::new(BULKHEAD.x, h + 0.38, z + 0.51),
    );
}

/// A door half: slab, raised ribs, a heavy seam edge carrying the lock
/// light, chevrons at its foot. Built around its own center.
fn bulkhead_half(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    kit: &Kit,
    lock: &Handle<StandardMaterial>,
    root: Entity,
    dir: f32,
) {
    let (w, h) = (BULKHEAD_SIZE.x * 0.5 + 0.02, BULKHEAD_SIZE.y);
    let seam = -dir * w * 0.5;
    let mut b = Build::<PaintMaterial>::default();
    b.cube(
        &kit.dark,
        Vec3::new(w, h, 0.18),
        Vec3::new(0.0, h * 0.5, 0.0),
    );
    for y in [0.95, 1.85, 2.75] {
        b.cube(
            &kit.steel,
            Vec3::new(w - 0.2, 0.18, 0.24),
            Vec3::new(dir * 0.06, y, 0.0),
        );
    }
    b.cube(
        &kit.steel,
        Vec3::new(0.2, h, 0.28),
        Vec3::new(seam + dir * 0.1, h * 0.5, 0.0),
    );
    b.cube(
        &kit.hazard,
        Vec3::new(w - 0.04, 0.42, 0.22),
        Vec3::new(0.0, 0.25, 0.0),
    );
    b.spawn(commands, meshes, Some(root), ());
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.035, h - 0.6, 0.03))),
        MeshMaterial3d(lock.clone()),
        Transform::from_xyz(seam + dir * 0.1, h * 0.5 + 0.1, 0.14),
        ChildOf(root),
        bevy::light::NotShadowCaster,
    ));
}

/// The corridor beyond the bulkhead: grated floor, ribs, a dim red strip
/// down its ceiling and a sealed door at the end under a red lamp.
fn corridor(b: &mut Build<PaintMaterial>, lit: &mut Build<StandardMaterial>, kit: &Kit) {
    let (w, h, z0, z1) = (
        BULKHEAD_SIZE.x + 0.4,
        3.4,
        ROOM_MIN.z - 0.3,
        ROOM_MIN.z - 7.0,
    );
    let mid = (z0 + z1) * 0.5;
    let len = z0 - z1;
    b.cube(
        &kit.floor,
        Vec3::new(w, 0.1, len),
        Vec3::new(0.0, -0.05, mid),
    );
    b.cube(
        &kit.dark,
        Vec3::new(w, 0.2, len),
        Vec3::new(0.0, h + 0.1, mid),
    );
    for side in [-1.0, 1.0] {
        b.cube(
            &kit.steel,
            Vec3::new(0.2, h, len),
            Vec3::new(side * (w * 0.5 + 0.1), h * 0.5, mid),
        );
    }
    b.cube(
        &kit.steel,
        Vec3::new(w, h, 0.2),
        Vec3::new(0.0, h * 0.5, z1 - 0.1),
    );
    let mut z = z0 - 0.9;
    while z > z1 + 0.5 {
        for side in [-1.0, 1.0] {
            b.cube(
                &kit.dark,
                Vec3::new(0.22, h, 0.3),
                Vec3::new(side * (w * 0.5 - 0.1), h * 0.5, z),
            );
        }
        b.cube(
            &kit.dark,
            Vec3::new(w, 0.22, 0.3),
            Vec3::new(0.0, h - 0.11, z),
        );
        z -= 1.6;
    }
    lit.cube(
        &kit.red_dim,
        Vec3::new(0.08, 0.03, len - 0.6),
        Vec3::new(0.0, h - 0.02, mid),
    );
    // The sealed door at the end.
    b.cube(
        &kit.dark,
        Vec3::new(1.6, 2.6, 0.1),
        Vec3::new(0.0, 1.3, z1 + 0.05),
    );
    b.cube(
        &kit.hazard,
        Vec3::new(1.6, 0.3, 0.12),
        Vec3::new(0.0, 0.15, z1 + 0.06),
    );
    lit.cube(
        &kit.red,
        Vec3::new(0.3, 0.08, 0.04),
        Vec3::new(0.0, 2.85, z1 + 0.12),
    );
}

/// The elevator's frame and the call panel's surround.
fn elevator_frame(b: &mut Build<PaintMaterial>, lit: &mut Build<StandardMaterial>, kit: &Kit) {
    let (w, h) = (ELEVATOR_SIZE.x, ELEVATOR_SIZE.y);
    let z = ROOM_MIN.z;
    for side in [-1.0, 1.0] {
        let x = ELEVATOR.x + side * (w * 0.5 + 0.12);
        b.cube(
            &kit.steel,
            Vec3::new(0.24, h + 0.15, 0.24),
            Vec3::new(x, (h + 0.15) * 0.5, z + 0.08),
        );
        lit.cube(
            &kit.amber_dim,
            Vec3::new(0.02, h - 0.4, 0.02),
            Vec3::new(x - side * 0.1, h * 0.5, z + 0.21),
        );
    }
    b.cube(
        &kit.steel,
        Vec3::new(w + 0.48, 0.15, 0.24),
        Vec3::new(ELEVATOR.x, h + 0.075, z + 0.08),
    );
    b.cube(
        &kit.dark,
        Vec3::new(w, 0.04, 0.3),
        Vec3::new(ELEVATOR.x, 0.02, z + 0.1),
    );
}

/// The car behind the elevator doors: a small lit room with a handrail.
fn car(
    b: &mut Build<PaintMaterial>,
    lit: &mut Build<StandardMaterial>,
    kit: &Kit,
    light: &Handle<StandardMaterial>,
) {
    let (w, h, z0, z1) = (
        ELEVATOR_SIZE.x + 0.1,
        2.95,
        ROOM_MIN.z - 0.3,
        ROOM_MIN.z - 2.4,
    );
    let (x, mid) = (ELEVATOR.x, (z0 + z1) * 0.5);
    let len = z0 - z1;
    b.cube(&kit.floor, Vec3::new(w, 0.1, len), Vec3::new(x, -0.05, mid));
    b.cube(
        &kit.dark,
        Vec3::new(w, 0.1, len),
        Vec3::new(x, h + 0.05, mid),
    );
    b.cube(
        &kit.steel,
        Vec3::new(w, h, 0.1),
        Vec3::new(x, h * 0.5, z1 - 0.05),
    );
    for side in [-1.0, 1.0] {
        b.cube(
            &kit.steel,
            Vec3::new(0.1, h, len),
            Vec3::new(x + side * (w * 0.5 + 0.05), h * 0.5, mid),
        );
    }
    lit.cube(
        light,
        Vec3::new(w - 0.5, 0.03, len - 0.6),
        Vec3::new(x, h - 0.02, mid),
    );
    lit.rod(
        &kit.pipe,
        0.025,
        Vec3::new(x - 0.8, 1.0, z1 + 0.1),
        Vec3::new(x + 0.8, 1.0, z1 + 0.1),
    );
}
