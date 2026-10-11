//! The weapons: the Arc Cutter, the Pulse Driver and the Rail Spike
//! (`models.rs`). The equipped one rides the engineer's gun pivot (the pose
//! puts his hands on its grips), with the `ammo` hologram (a `HoloAnchor`)
//! floating over it and its tri-laser sight lit while he aims. A shot
//! throws a bolt down the room that bursts into sparks where it strikes; a
//! reload racks the weapon (the pose plays it from `GunKick`).

pub mod models;

pub use models::spawn_weapon_model;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_react::{ReactAppExt, react_message};

use crate::doors::Bulkhead;
use crate::engineer::{GunKick, Rig, Stance};
use crate::holo::HoloAnchor;
use crate::layout::{BULKHEAD, BULKHEAD_SIZE, ROOM_MIN};
use crate::station::kit::additive;
use crate::station::{Burst, BurstKind, Flash, Noise};

/// React → Bevy: put this weapon in his hands (`cutter`, `pulse`, `rail`).
#[react_message(name = "weapon.equip")]
pub struct Equip {
    pub weapon: String,
}

/// React → Bevy: one shot (React already took the round off the clip).
#[react_message(name = "weapon.fire")]
pub struct Fire;

/// React → Bevy: reload.
#[react_message(name = "weapon.reload")]
pub struct Reload;

pub const WEAPONS: [&str; 3] = ["cutter", "pulse", "rail"];

/// The models in his hands (in `WEAPONS` order), their lasers and muzzle
/// flashes, which one is equipped.
#[derive(Resource)]
struct Arsenal {
    models: [Entity; 3],
    lasers: [Entity; 3],
    flashes: [Entity; 3],
    anchor: Entity,
    equipped: usize,
    /// Seconds left on the muzzle flash.
    flash: f32,
    bolt_mesh: Handle<Mesh>,
    bolt_looks: [Handle<StandardMaterial>; 3],
}

#[derive(Component)]
struct Bolt {
    velocity: Vec3,
    stop_z: f32,
}

/// How each weapon's shot flies: its speed (m/s), its shape (meters).
const BOLTS: [(f32, Vec3); 3] = [
    (34.0, Vec3::new(0.2, 0.014, 0.08)),
    (60.0, Vec3::new(0.018, 0.018, 0.34)),
    (30.0, Vec3::new(0.016, 0.016, 0.62)),
];
/// How far the sight's beams reach.
const LASER_LEN: f32 = 8.0;

pub struct WeaponsPlugin;

impl Plugin for WeaponsPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_systems(
            Update,
            (arm, (show, fly).run_if(resource_exists::<Arsenal>)),
        );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_equip)
        .add_react_handler(on_fire)
        .add_react_handler(on_reload);
}

/// Hang the three models, their lasers and the ammo anchor on the gun
/// pivot, once the engineer stands.
fn arm(
    mut commands: Commands,
    rigs: Query<&Rig, Added<Rig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(rig) = rigs.iter().next() else {
        return;
    };
    let beam = additive(&mut materials, LinearRgba::rgb(0.25, 1.4, 2.2));
    let beam_mesh = meshes.add(Cuboid::new(0.0035, 0.0035, LASER_LEN));
    let flash_mesh = meshes.add(Sphere::new(1.0).mesh().uv(12, 8));
    let flash_look = additive(&mut materials, LinearRgba::rgb(4.0, 6.0, 8.0));
    let mut models = [Entity::PLACEHOLDER; 3];
    let mut lasers = [Entity::PLACEHOLDER; 3];
    let mut flashes = [Entity::PLACEHOLDER; 3];
    for (i, weapon) in WEAPONS.iter().enumerate() {
        let model = spawn_weapon_model(
            &mut commands,
            &mut meshes,
            &mut materials,
            weapon,
            RenderLayers::default(),
        );
        let spec = models::spec(weapon);
        commands.entity(model).insert((
            ChildOf(rig.gun),
            if i == 0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ));
        let laser = commands
            .spawn((Transform::default(), Visibility::Hidden, ChildOf(model)))
            .id();
        for &at in spec.lasers {
            commands.spawn((
                Mesh3d(beam_mesh.clone()),
                MeshMaterial3d(beam.clone()),
                Transform::from_translation(at - Vec3::Z * LASER_LEN * 0.5),
                bevy::light::NotShadowCaster,
                ChildOf(laser),
            ));
        }
        let flash = commands
            .spawn((
                Mesh3d(flash_mesh.clone()),
                MeshMaterial3d(flash_look.clone()),
                Transform::from_translation(spec.muzzle).with_scale(Vec3::new(0.05, 0.05, 0.09)),
                Visibility::Hidden,
                bevy::light::NotShadowCaster,
                ChildOf(model),
            ))
            .id();
        models[i] = model;
        lasers[i] = laser;
        flashes[i] = flash;
    }
    let anchor = commands
        .spawn((
            HoloAnchor("ammo"),
            Transform::from_translation(models::CUTTER.ammo),
            Visibility::default(),
            ChildOf(rig.gun),
        ))
        .id();
    let bolt_looks = [
        additive(&mut materials, LinearRgba::rgb(2.0, 7.0, 10.0)),
        additive(&mut materials, LinearRgba::rgb(9.0, 4.0, 1.2)),
        additive(&mut materials, LinearRgba::rgb(8.0, 3.6, 0.8)),
    ];
    commands.insert_resource(Arsenal {
        models,
        lasers,
        flashes,
        anchor,
        equipped: 0,
        flash: 0.0,
        bolt_mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        bolt_looks,
    });
}

fn on_equip(
    equip: On<Equip>,
    arsenal: Option<ResMut<Arsenal>>,
    mut visibility: Query<&mut Visibility>,
    mut transforms: Query<&mut Transform>,
) {
    let (Some(mut arsenal), Some(i)) = (arsenal, WEAPONS.iter().position(|w| *w == equip.weapon))
    else {
        return;
    };
    arsenal.equipped = i;
    for (j, model) in arsenal.models.iter().enumerate() {
        if let Ok(mut v) = visibility.get_mut(*model) {
            v.set_if_neq(if j == i {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
    if let Ok(mut t) = transforms.get_mut(arsenal.anchor) {
        t.translation = models::spec(WEAPONS[i]).ammo;
    }
}

#[allow(clippy::too_many_arguments)]
fn on_fire(
    _: On<Fire>,
    arsenal: Option<ResMut<Arsenal>>,
    bulkhead: Option<Res<Bulkhead>>,
    mut kick: ResMut<GunKick>,
    mut commands: Commands,
    globals: Query<&GlobalTransform>,
    mut flashes: MessageWriter<Flash>,
) {
    let Some(mut arsenal) = arsenal else {
        return;
    };
    let i = arsenal.equipped;
    let Ok(model) = globals.get(arsenal.models[i]) else {
        return;
    };
    kick.recoil = 1.0;
    arsenal.flash = 0.06;
    let spec = models::spec(WEAPONS[i]);
    let muzzle = model.transform_point(spec.muzzle);
    let dir = (model.rotation() * Vec3::NEG_Z).normalize();
    let (speed, size) = BOLTS[i];
    // Through the bulkhead's opening if it stands open, else the far wall.
    let open = bulkhead.is_some_and(|b| b.is_open());
    let through = open && (muzzle.x - BULKHEAD.x).abs() < BULKHEAD_SIZE.x * 0.5 - 0.1;
    let stop_z = if through {
        ROOM_MIN.z - 6.9
    } else {
        ROOM_MIN.z + 0.05
    };
    commands.spawn((
        Mesh3d(arsenal.bolt_mesh.clone()),
        MeshMaterial3d(arsenal.bolt_looks[i].clone()),
        Transform::from_translation(muzzle)
            .looking_to(dir, Vec3::Y)
            .with_scale(size),
        Bolt {
            velocity: dir * speed,
            stop_z,
        },
        bevy::light::NotShadowCaster,
    ));
    let color = [
        LinearRgba::rgb(0.5, 0.85, 1.0),
        LinearRgba::rgb(1.0, 0.6, 0.25),
        LinearRgba::rgb(1.0, 0.55, 0.2),
    ][i];
    flashes.write(Flash {
        at: muzzle + dir * 0.1,
        color,
        intensity: 350_000.0,
        secs: 0.12,
    });
}

fn on_reload(_: On<Reload>, mut kick: ResMut<GunKick>) {
    kick.reload = Some(0.0);
}

/// The sight while aiming, the muzzle flash's moment.
fn show(
    time: Res<Time>,
    stance: Res<Stance>,
    mut arsenal: ResMut<Arsenal>,
    mut visibility: Query<&mut Visibility>,
) {
    arsenal.flash = (arsenal.flash - time.delta_secs()).max(0.0);
    for i in 0..3 {
        let on = i == arsenal.equipped;
        if let Ok(mut v) = visibility.get_mut(arsenal.lasers[i]) {
            v.set_if_neq(if on && stance.aim > 0.85 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
        if let Ok(mut v) = visibility.get_mut(arsenal.flashes[i]) {
            v.set_if_neq(if on && arsenal.flash > 0.0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
}

/// Fly the bolts; where one strikes, sparks and a flash.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &Bolt, &mut Transform)>,
    mut bursts: MessageWriter<Burst>,
    mut flashes: MessageWriter<Flash>,
    mut noise: MessageWriter<Noise>,
) {
    for (e, bolt, mut t) in &mut bolts {
        t.translation += bolt.velocity * time.delta_secs();
        // The far wall, or the floor (a shot fired while the gun is still
        // coming up).
        let floor = t.translation.y <= 0.0;
        if t.translation.z > bolt.stop_z && !floor {
            continue;
        }
        let at = if floor {
            t.translation.with_y(0.02)
        } else {
            t.translation.with_z(bolt.stop_z)
        };
        bursts.write(Burst {
            at,
            dir: Vec3::new(0.0, 0.35, 1.0),
            kind: BurstKind::Sparks,
        });
        flashes.write(Flash {
            at: at + Vec3::Z * 0.4,
            color: LinearRgba::rgb(1.0, 0.7, 0.4),
            intensity: 260_000.0,
            secs: 0.18,
        });
        noise.write(Noise("impact", at));
        commands.entity(e).despawn();
    }
}
