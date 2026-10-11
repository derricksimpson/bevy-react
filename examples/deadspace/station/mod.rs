//! The deck: Maintenance Concourse C of Eurydice Station. A ribbed steel
//! room (`layout.rs` has the floor plan), lit by two flickering ceiling
//! strips and a rotating emergency beacon, filled with drifting dust and
//! the odd burst of steam, and the fixtures the holograms hang on.
//!
//! The light budget is eight (WebGL2, and the frame time): five strip
//! lights (the one over the far wall a shadow-casting spot), the beacon,
//! the bench lamp, and `fx`'s shared flash. Everything else that shines is
//! emissive and blooms.

pub mod fixtures;
pub mod fx;
pub mod kit;
pub mod shell;

use bevy::prelude::*;

pub use fx::{Burst, BurstKind, Flash};
use kit::{Kit, PaintMaterial};

/// A strip light that now and then stutters, with the diffuser segment
/// that goes dark with it.
#[derive(Component)]
struct Stutter {
    base: f32,
    diffuser: Option<(Handle<StandardMaterial>, LinearRgba)>,
    /// Seconds until the next stutter, or left in this one (negative).
    clock: f32,
    /// The level shown, and for how much longer.
    level: f32,
    hold: f32,
    rng: u32,
}

/// Make the strip light nearest `near` stutter for `secs` (a shock, a
/// slammed door).
#[derive(Message, Clone, Copy)]
pub struct Flicker {
    pub near: Vec3,
    pub secs: f32,
}

/// Something in the world made a noise worth a sound (the sound module
/// listens): `"bulkheadOpen"`, `"elevatorDoors"`, `"elevatorMove"`,
/// `"elevatorArrive"`, `"impact"`, `"stasisHit"`, at a place. (A shocked hack
/// panel's crackle is React's `shock` cue.)
#[derive(Message, Clone, Copy)]
pub struct Noise(pub &'static str, pub Vec3);

#[derive(Component)]
struct Beacon;

/// How the strip lights sit: (position, is the shadow spot).
const STRIP_LIGHTS: [(Vec3, bool); 5] = [
    (Vec3::new(0.0, 4.05, -2.3), true),
    (Vec3::new(-6.4, 3.95, -2.6), false),
    (Vec3::new(6.4, 3.95, -2.6), false),
    (Vec3::new(-3.6, 3.95, 3.2), false),
    (Vec3::new(3.6, 3.95, 3.2), false),
];
const STRIP_COLOR: Color = Color::srgb(0.8, 0.9, 1.0);

pub struct StationPlugin;

impl Plugin for StationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PaintMaterial>::default())
            .init_resource::<Kit>()
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.45, 0.68, 0.72),
                brightness: 90.0,
                ..default()
            })
            .add_message::<Flicker>()
            .add_message::<Noise>()
            .add_systems(Startup, build)
            .add_systems(Update, (stutter, sweep_beacon));
        fx::plugin(app);
    }
}

fn build(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    kit: Res<Kit>,
) {
    let shell = shell::build(&mut commands, &mut meshes, &mut materials, &kit);
    fixtures::build(&mut commands, &mut meshes, &kit);

    // The strip lights; the two over a flickering diffuser stutter with it.
    for (i, (at, shadows)) in STRIP_LIGHTS.into_iter().enumerate() {
        let diffuser = match i {
            0 => shell.flicker.first(),
            4 => shell.flicker.get(1),
            _ => None,
        }
        .map(|h| (h.clone(), LinearRgba::rgb(3.2, 3.6, 3.8)));
        let stutter = Stutter {
            base: if shadows { 2_400_000.0 } else { 700_000.0 },
            diffuser,
            clock: 3.0 + i as f32 * 2.3,
            level: 1.0,
            hold: 0.0,
            rng: 0x9e37_79b9 ^ (i as u32).wrapping_mul(0x85eb_ca6b).wrapping_add(1),
        };
        if shadows {
            commands.spawn((
                SpotLight {
                    color: STRIP_COLOR,
                    intensity: stutter.base,
                    range: 16.0,
                    radius: 0.2,
                    shadow_maps_enabled: true,
                    outer_angle: 1.2,
                    inner_angle: 0.55,
                    ..default()
                },
                Transform::from_translation(at).looking_at(Vec3::new(0.0, 0.0, -2.9), Vec3::Z),
                stutter,
            ));
        } else {
            commands.spawn((
                PointLight {
                    color: STRIP_COLOR,
                    intensity: stutter.base,
                    range: 10.0,
                    radius: 0.25,
                    ..default()
                },
                Transform::from_translation(at),
                stutter,
            ));
        }
    }

    // The emergency beacon over the hack panel: a wall bracket, an amber
    // dome in a wire cage, and a reflector + spot light turning inside.
    let at = Vec3::new(-1.95, 3.45, -4.78);
    let mut b = kit::Build::<StandardMaterial>::default();
    b.cube(
        &kit.rubber,
        Vec3::new(0.16, 0.05, 0.2),
        at - Vec3::new(0.0, 0.03, 0.06),
    );
    b.add(
        &kit.pipe,
        Cylinder::new(0.07, 0.04).mesh().resolution(16),
        Transform::from_translation(at),
    );
    b.add(
        &kit.amber,
        Cylinder::new(0.058, 0.09).mesh().resolution(16),
        Transform::from_translation(at + Vec3::Y * 0.065),
    );
    b.add(
        &kit.amber,
        Sphere::new(0.058).mesh().uv(16, 8),
        Transform::from_translation(at + Vec3::Y * 0.11),
    );
    for y in [0.05, 0.1] {
        b.add(
            &kit.pipe,
            Torus::new(0.06, 0.068)
                .mesh()
                .minor_resolution(4)
                .major_resolution(16),
            Transform::from_translation(at + Vec3::Y * y),
        );
    }
    b.spawn(
        &mut commands,
        &mut meshes,
        None,
        bevy::light::NotShadowCaster,
    );
    commands
        .spawn((
            Transform::from_translation(at + Vec3::Y * 0.08),
            Visibility::default(),
            Beacon,
        ))
        .with_children(|pivot| {
            pivot.spawn((
                SpotLight {
                    color: Color::srgb(1.0, 0.42, 0.08),
                    intensity: 900_000.0,
                    range: 14.0,
                    radius: 0.05,
                    outer_angle: 0.38,
                    inner_angle: 0.12,
                    ..default()
                },
                Transform::default().looking_to(Vec3::new(1.0, -0.22, 0.0), Vec3::Y),
            ));
            pivot.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.012, 0.07, 0.07))),
                MeshMaterial3d(kit.white.clone()),
                Transform::from_xyz(-0.03, 0.0, 0.0),
                bevy::light::NotShadowCaster,
            ));
        });

    // The bench's work lamp.
    commands.spawn((
        SpotLight {
            color: Color::srgb(1.0, 0.82, 0.6),
            intensity: 450_000.0,
            range: 6.0,
            radius: 0.06,
            outer_angle: 0.9,
            inner_angle: 0.4,
            ..default()
        },
        Transform::from_xyz(8.6, 2.6, -0.55).looking_at(Vec3::new(9.3, 0.9, -0.5), Vec3::Y),
    ));
}

/// Now and then a strip light stutters — a run of short dropouts — and its
/// diffuser goes dark with it. A [`Flicker`] starts one at once.
fn stutter(
    time: Res<Time>,
    mut kicks: MessageReader<Flicker>,
    mut lights: Query<(
        &GlobalTransform,
        &mut Stutter,
        Option<&mut PointLight>,
        Option<&mut SpotLight>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for kick in kicks.read() {
        if let Some((_, mut s, _, _)) = lights.iter_mut().min_by(|a, b| {
            a.0.translation()
                .distance(kick.near)
                .total_cmp(&b.0.translation().distance(kick.near))
        }) {
            s.clock = -kick.secs;
        }
    }
    let dt = time.delta_secs();
    for (_, mut s, point, spot) in &mut lights {
        if s.clock > 0.0 {
            s.clock -= dt;
            if s.clock <= 0.0 {
                // A stutter of 0.3–1.2 s.
                s.clock = -(0.3 + 0.9 * fx::rand(&mut s.rng));
            }
            s.level = 1.0;
        } else {
            s.clock += dt;
            s.hold -= dt;
            if s.clock >= 0.0 {
                s.clock = 5.0 + 9.0 * fx::rand(&mut s.rng);
                s.level = 1.0;
            } else if s.hold <= 0.0 {
                // Each dropout or half-glow holds a few frames.
                s.hold = 0.03 + 0.07 * fx::rand(&mut s.rng);
                s.level = if fx::rand(&mut s.rng) < 0.45 {
                    0.05
                } else {
                    0.4 + 0.6 * fx::rand(&mut s.rng)
                };
            }
        }
        let level = s.level;
        let intensity = s.base * level;
        if let Some(mut p) = point
            && p.intensity != intensity
        {
            p.intensity = intensity;
        }
        if let Some(mut p) = spot
            && p.intensity != intensity
        {
            p.intensity = intensity;
        }
        if let Some((handle, base)) = &s.diffuser {
            let emissive = *base * level;
            if let Some(m) = materials.get(handle)
                && m.emissive != emissive
                && let Some(mut m) = materials.get_mut(handle)
            {
                m.emissive = emissive;
            }
        }
    }
}

fn sweep_beacon(time: Res<Time>, mut beacon: Query<&mut Transform, With<Beacon>>) {
    for mut t in &mut beacon {
        t.rotation = Quat::from_rotation_y(time.elapsed_secs() * 2.4);
    }
}
