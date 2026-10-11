//! The deck miniature for the suit menu's MAP tab: the room's outline, its
//! fixtures and the engineer drawn as glowing lines over a dark plate, the
//! objective marked by an amber beam. The camera orbits it: React's drags
//! turn it (`map.orbit`), the wheel zooms (`map.zoom`).
//!
//! Built 1:1 from `layout.rs`, far below the deck (see the module above).
//! The target clears opaque dark, so additive lines can glow on it.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_react::{RenderTargetSpec, RenderTargets};

use super::{MAP_AT, MAP_LAYER, MarkMap, OrbitMap, ZoomMap, portal_camera};
use crate::layout;

/// The map's orbit: around the room's middle.
#[derive(Component)]
pub(super) struct MapCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

/// The objective's beam.
#[derive(Component)]
pub(super) struct Marker;

/// The bulkhead's block, and the material it takes once unlocked.
#[derive(Component)]
pub(super) struct Bulkhead(Handle<StandardMaterial>);

const CENTER: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const PLATE: Color = Color::srgb(0.008, 0.03, 0.045);

pub(super) fn spawn_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut targets: ResMut<RenderTargets>,
) {
    let layers = RenderLayers::layer(MAP_LAYER);
    let target = targets.create(&mut images, "map", RenderTargetSpec::default());
    let cam = MapCamera {
        yaw: 0.35,
        pitch: 0.95,
        distance: 24.0,
    };
    let (eye, look) = orbit(&cam);
    let mut camera = commands.spawn(portal_camera(
        "map",
        eye,
        look,
        34.0,
        &layers,
        target.camera_target(),
    ));
    camera.insert(cam);
    camera.entry::<Camera>().and_modify(|mut c| {
        c.clear_color = ClearColorConfig::Custom(PLATE);
    });

    let glow = |materials: &mut Assets<StandardMaterial>, c: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: Color::LinearRgba(c),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        })
    };
    // The legend's colors (`ui/src/holos/menu/MapTab.tsx`).
    let line = glow(&mut materials, LinearRgba::rgb(0.25, 0.75, 0.95));
    let faint = glow(&mut materials, LinearRgba::rgb(0.03, 0.1, 0.13));
    let fixture = glow(&mut materials, LinearRgba::rgb(0.12, 0.35, 0.42));
    let cyan = glow(&mut materials, LinearRgba::rgb(0.35, 0.9, 1.0));
    let white = glow(&mut materials, LinearRgba::rgb(0.9, 1.0, 1.0));
    let amber = glow(&mut materials, LinearRgba::rgb(1.0, 0.55, 0.12));
    let green = glow(&mut materials, LinearRgba::rgb(0.3, 1.0, 0.5));
    let red = glow(&mut materials, LinearRgba::rgb(1.0, 0.18, 0.12));
    let cube = meshes.add(Cuboid::from_length(1.0));
    let bar =
        |commands: &mut Commands, material: &Handle<StandardMaterial>, center: Vec3, size: Vec3| {
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(MAP_AT + center).with_scale(size),
                layers.clone(),
            ));
        };

    let (min, max) = (layout::ROOM_MIN, layout::ROOM_MAX);
    // The floor grid, every 2 m.
    let mut x = min.x;
    while x <= max.x + 0.01 {
        bar(
            &mut commands,
            &faint,
            Vec3::new(x, 0.0, (min.z + max.z) * 0.5),
            Vec3::new(0.04, 0.01, max.z - min.z),
        );
        x += 2.0;
    }
    let mut z = min.z;
    while z <= max.z + 0.01 {
        bar(
            &mut commands,
            &faint,
            Vec3::new(0.0, 0.0, z),
            Vec3::new(max.x - min.x, 0.01, 0.04),
        );
        z += 2.0;
    }
    // The walls: low outlines, the far wall broken by the bulkhead and the
    // elevator.
    let h = 0.8;
    let far = min.z;
    let bulk = layout::BULKHEAD_SIZE.x * 0.5;
    let lift = (
        layout::ELEVATOR.x - layout::ELEVATOR_SIZE.x * 0.5,
        layout::ELEVATOR.x + layout::ELEVATOR_SIZE.x * 0.5,
    );
    for (a, b) in [
        (Vec3::new(min.x, 0.0, far), Vec3::new(-bulk, 0.0, far)),
        (Vec3::new(bulk, 0.0, far), Vec3::new(lift.0, 0.0, far)),
        (Vec3::new(lift.1, 0.0, far), Vec3::new(max.x, 0.0, far)),
        (Vec3::new(min.x, 0.0, max.z), Vec3::new(max.x, 0.0, max.z)),
        (Vec3::new(min.x, 0.0, far), Vec3::new(min.x, 0.0, max.z)),
        (Vec3::new(max.x, 0.0, far), Vec3::new(max.x, 0.0, max.z)),
        // The corridor beyond the bulkhead, and the elevator shaft.
        (Vec3::new(-bulk, 0.0, far - 4.0), Vec3::new(-bulk, 0.0, far)),
        (Vec3::new(bulk, 0.0, far - 4.0), Vec3::new(bulk, 0.0, far)),
        (
            Vec3::new(lift.0, 0.0, far - 2.2),
            Vec3::new(lift.0, 0.0, far),
        ),
        (
            Vec3::new(lift.1, 0.0, far - 2.2),
            Vec3::new(lift.1, 0.0, far),
        ),
        (
            Vec3::new(lift.0, 0.0, far - 2.2),
            Vec3::new(lift.1, 0.0, far - 2.2),
        ),
    ] {
        let mid = (a + b) * 0.5 + Vec3::Y * h * 0.5;
        bar(
            &mut commands,
            &line,
            mid,
            (b - a).abs() + Vec3::new(0.12, h, 0.12),
        );
    }
    // The fixtures as solid blocks against their walls.
    for (at, size, material) in [
        (
            layout::KIOSK + Vec3::new(0.4, 1.0, 0.0),
            Vec3::new(0.8, 2.0, 1.6),
            &amber,
        ),
        (
            layout::BENCH + Vec3::new(-0.5, 0.5, 0.0),
            Vec3::new(1.0, 1.0, 2.4),
            &green,
        ),
        (
            layout::TERMINAL + Vec3::new(0.3, 0.8, 0.0),
            Vec3::new(0.6, 1.6, 1.4),
            &fixture,
        ),
        (
            layout::SAVE_STATION + Vec3::new(0.0, 0.9, 0.3),
            Vec3::new(0.7, 1.8, 0.6),
            &cyan,
        ),
        (
            layout::HACK_PANEL + Vec3::new(0.0, 1.3, 0.15),
            Vec3::new(0.5, 0.6, 0.3),
            &fixture,
        ),
        (
            layout::ELEVATOR + Vec3::new(0.0, 1.4, -1.1),
            Vec3::new(1.9, 2.8, 2.0),
            &white,
        ),
    ] {
        bar(&mut commands, material, at, size);
    }
    // The bulkhead: red while sealed, green once the hack opens it.
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(red),
        Transform::from_translation(MAP_AT + layout::BULKHEAD + Vec3::Y * 1.6).with_scale(
            Vec3::new(layout::BULKHEAD_SIZE.x, layout::BULKHEAD_SIZE.y, 0.25),
        ),
        layers.clone(),
        Bulkhead(green),
    ));
    // You: a white arrow pointing the way you face.
    commands.spawn((
        Mesh3d(meshes.add(Cone::new(0.55, 1.2))),
        MeshMaterial3d(white.clone()),
        Transform::from_translation(MAP_AT + layout::ENGINEER + Vec3::Y * 0.3)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::new(1.0, 1.0, 0.3)),
        layers.clone(),
    ));
    // The objective's beam, hidden until React marks one.
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(amber),
        Transform::from_translation(MAP_AT).with_scale(Vec3::new(0.35, 6.0, 0.35)),
        Visibility::Hidden,
        layers.clone(),
        Marker,
    ));
}

/// Eye and look-at for an orbit.
fn orbit(cam: &MapCamera) -> (Vec3, Vec3) {
    let look = MAP_AT + CENTER;
    let dir = Vec3::new(
        cam.yaw.sin() * cam.pitch.cos(),
        cam.pitch.sin(),
        cam.yaw.cos() * cam.pitch.cos(),
    );
    (look + dir * cam.distance, look)
}

pub(super) fn on_orbit(drag: On<OrbitMap>, mut cam: Single<&mut MapCamera>) {
    cam.yaw -= drag.dx * 0.008;
    cam.pitch = (cam.pitch + drag.dy * 0.006).clamp(0.2, 1.45);
}

pub(super) fn on_zoom(zoom: On<ZoomMap>, mut cam: Single<&mut MapCamera>) {
    cam.distance = (cam.distance * 0.9f32.powf(zoom.delta)).clamp(9.0, 40.0);
}

pub(super) fn on_mark(
    mark: On<MarkMap>,
    mut marker: Single<(&mut Transform, &mut Visibility), With<Marker>>,
) {
    let (transform, visibility) = &mut *marker;
    match layout::destination(&mark.to) {
        Some(at) => {
            transform.translation = MAP_AT + at + Vec3::Y * 3.0;
            **visibility = Visibility::Inherited;
        }
        None => **visibility = Visibility::Hidden,
    }
}

/// The hack opened the bulkhead: its block turns green.
pub(super) fn on_unlock(
    _: On<crate::doors::UnlockDoor>,
    mut bulkhead: Single<(&Bulkhead, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let (open, material) = &mut *bulkhead;
    material.0 = open.0.clone();
}

/// Follow the orbit: only when a drag or the wheel changed it.
pub(super) fn orbit_camera(mut cameras: Query<(&MapCamera, &mut Transform), Changed<MapCamera>>) {
    for (cam, mut transform) in &mut cameras {
        let (eye, look) = orbit(cam);
        *transform = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    }
}

/// The objective's beam breathes.
pub(super) fn pulse_marker(time: Res<Time>, mut marker: Query<&mut Transform, With<Marker>>) {
    let k = 1.0 + 0.25 * (time.elapsed_secs() * 3.0).sin();
    for mut transform in &mut marker {
        transform.scale = Vec3::new(0.35 * k, 6.0, 0.35 * k);
    }
}
