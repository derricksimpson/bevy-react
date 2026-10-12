//! The engineer: a hard-surface suit built from code (`suit.rs`), walking
//! where the player goes and posed every frame (`pose.rs`). He carries the
//! worn holograms (`HoloAnchor`s for `spine`, `stasis` and `air`, and for
//! the `menu` and the `prompt` he projects) and answers the suit messages
//! React sends: a hit
//! makes him flinch in a spray of sparks, a medpack runs a ring of green
//! light up the suit, stasis flies off his left hand, a vacuum fogs his
//! helmet and pales his lights.

mod limbs;
pub mod pose;
pub mod suit;

use bevy::prelude::*;
use bevy_react::{ReactAppExt, react_message};

pub use pose::GunKick;
pub use suit::Rig;

use crate::layout;
use crate::player::{Player, PlayerSet};
use crate::station::kit::{PaintMaterial, additive};
use crate::station::{Burst, BurstKind, Flash, Noise};
use pose::Reactions;
use suit::{SUIT_LIGHT, SuitLights};

/// React → Bevy: the suit took a hit (the spine already shows the damage).
#[react_message(name = "suit.hit")]
pub struct SuitHit;

/// React → Bevy: a medpack was used.
#[react_message(name = "suit.heal")]
pub struct SuitHeal;

/// React → Bevy: the stasis module fired.
#[react_message(name = "suit.stasis")]
pub struct SuitStasis;

/// React → Bevy: the room lost (or got back) its air.
#[react_message(name = "suit.vacuum")]
pub struct SuitVacuum {
    pub on: bool,
}

/// The heal ring's run (0..1) and the vacuum's state, for the lights.
#[derive(Resource, Default)]
struct Glow {
    heal: Option<f32>,
    vacuum: bool,
    /// The stasis bolt leaves the hand this long after the message.
    throw_in: Option<f32>,
}

#[derive(Component)]
struct HealRing;

#[derive(Component)]
struct StasisBolt {
    velocity: Vec3,
    /// Meters left to fly.
    left: f32,
}

/// The stasis bolt's look, made once.
#[derive(Resource)]
struct BoltAssets {
    mesh: Handle<Mesh>,
    core: Handle<StandardMaterial>,
    halo: Handle<StandardMaterial>,
}

pub struct EngineerPlugin;

impl Plugin for EngineerPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.init_resource::<GunKick>()
            .init_resource::<Reactions>()
            .init_resource::<Glow>()
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                (follow, pose::pose, light_up, fly_bolts).after(PlayerSet),
            );
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_hit)
        .add_react_handler(on_heal)
        .add_react_handler(on_stasis)
        .add_react_handler(on_vacuum);
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut paints: ResMut<Assets<PaintMaterial>>,
) {
    suit::spawn(&mut commands, &mut meshes, &mut materials, &mut paints);
    commands.spawn((
        Mesh3d(
            meshes.add(
                Torus::new(0.3, 0.33)
                    .mesh()
                    .minor_resolution(8)
                    .major_resolution(48),
            ),
        ),
        MeshMaterial3d(additive(&mut materials, LinearRgba::rgb(0.6, 5.0, 1.4))),
        Transform::default(),
        Visibility::Hidden,
        HealRing,
        bevy::light::NotShadowCaster,
    ));
    commands.insert_resource(BoltAssets {
        mesh: meshes.add(Sphere::new(1.0).mesh().uv(16, 8)),
        core: additive(&mut materials, LinearRgba::rgb(3.0, 8.0, 16.0)),
        halo: additive(&mut materials, LinearRgba::rgb(0.06, 0.25, 0.7)),
    });
}

/// He stands where the player is, facing the player's way.
fn follow(player: Res<Player>, mut suits: Query<&mut Transform, With<Rig>>) {
    for mut suit in &mut suits {
        *suit = player.frame();
    }
}

fn on_hit(
    _: On<SuitHit>,
    time: Res<Time>,
    player: Res<Player>,
    mut react: ResMut<Reactions>,
    mut bursts: MessageWriter<Burst>,
    mut flashes: MessageWriter<Flash>,
) {
    react.flinch = 1.0;
    react.flinch_roll = ((time.elapsed_secs() * 13.7).sin()) * 0.18;
    // Off his back, toward the camera.
    let body = player.frame();
    let at = body.transform_point(Vec3::new(0.12, 1.45, 0.2));
    bursts.write(Burst {
        at,
        dir: body.rotation * Vec3::new(0.3, 0.5, 1.0),
        kind: BurstKind::Sparks,
    });
    flashes.write(Flash {
        at: at + body.rotation * Vec3::Z * 0.4,
        color: LinearRgba::rgb(1.0, 0.55, 0.25),
        intensity: 250_000.0,
        secs: 0.2,
    });
}

fn on_heal(
    _: On<SuitHeal>,
    player: Res<Player>,
    mut glow: ResMut<Glow>,
    mut bursts: MessageWriter<Burst>,
) {
    glow.heal = Some(0.0);
    bursts.write(Burst {
        at: player.at + Vec3::new(0.0, 1.1, 0.0),
        dir: Vec3::Y,
        kind: BurstKind::Heal,
    });
}

fn on_stasis(_: On<SuitStasis>, mut react: ResMut<Reactions>, mut glow: ResMut<Glow>) {
    react.throw = Some(0.0);
    glow.throw_in = Some(0.2);
}

fn on_vacuum(
    vacuum: On<SuitVacuum>,
    player: Res<Player>,
    mut glow: ResMut<Glow>,
    mut bursts: MessageWriter<Burst>,
) {
    // React restates the vacuum on mount and on a load: only a change puffs.
    if glow.vacuum == vacuum.on {
        return;
    }
    glow.vacuum = vacuum.on;
    bursts.write(Burst {
        at: player.at + Vec3::new(0.0, 1.66, 0.0),
        dir: Vec3::Y,
        kind: BurstKind::Frost,
    });
}

/// The heal ring rising up the suit, the lights' tint (green while
/// healing, pale in a vacuum), and the stasis bolt leaving the hand.
#[allow(clippy::too_many_arguments)]
fn light_up(
    mut commands: Commands,
    time: Res<Time>,
    player: Res<Player>,
    mut glow: ResMut<Glow>,
    lights: Option<Res<SuitLights>>,
    bolt: Option<Res<BoltAssets>>,
    bulkhead: Option<Res<crate::doors::Bulkhead>>,
    rigs: Query<&Rig>,
    globals: Query<&GlobalTransform>,
    mut rings: Query<(&mut Transform, &mut Visibility), With<HealRing>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flashes: MessageWriter<Flash>,
) {
    let dt = time.delta_secs();
    let mut tint = if glow.vacuum {
        LinearRgba::rgb(1.4, 2.6, 4.6)
    } else {
        SUIT_LIGHT
    };
    if let Some(h) = glow.heal {
        let h = h + dt / 0.9;
        glow.heal = (h < 1.0).then_some(h);
        let k = (h.min(1.0) * std::f32::consts::PI).sin();
        tint = mix(tint, LinearRgba::rgb(0.8, 6.0, 1.6), k);
        for (mut t, mut v) in &mut rings {
            t.translation = player.at + Vec3::Y * (0.08 + 1.85 * h);
            t.scale = Vec3::splat(1.25 - 0.45 * h);
            v.set_if_neq(if h < 1.0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
    if let Some(lights) = lights
        && materials.get(&lights.0).is_some_and(|m| m.emissive != tint)
        && let Some(mut m) = materials.get_mut(&lights.0)
    {
        m.emissive = tint;
    }

    // The stasis bolt leaves the left hand once the arm is out.
    let Some(left) = glow.throw_in.map(|s| s - dt) else {
        return;
    };
    glow.throw_in = (left > 0.0).then_some(left);
    if left > 0.0 {
        return;
    }
    let (Some(bolt), Some(rig)) = (bolt, rigs.iter().next()) else {
        return;
    };
    let Ok(hand) = globals.get(rig.arms[1][2]) else {
        return;
    };
    // Where he looks, as far as the room goes.
    let dir = player.view() * Vec3::NEG_Z;
    let from = hand.translation() + dir * 0.12 - Vec3::Y * 0.08;
    let left = layout::reach(from, dir, bulkhead.is_some_and(|b| b.is_open())) - 0.1;
    commands
        .spawn((
            Transform::from_translation(from).looking_to(dir, Vec3::Y),
            Visibility::default(),
            StasisBolt {
                velocity: dir * 13.0,
                left,
            },
        ))
        .with_children(|b| {
            b.spawn((
                Mesh3d(bolt.mesh.clone()),
                MeshMaterial3d(bolt.core.clone()),
                Transform::from_scale(Vec3::new(0.05, 0.05, 0.11)),
                bevy::light::NotShadowCaster,
            ));
            b.spawn((
                Mesh3d(bolt.mesh.clone()),
                MeshMaterial3d(bolt.halo.clone()),
                Transform::from_scale(Vec3::new(0.16, 0.16, 0.3)),
                bevy::light::NotShadowCaster,
            ));
        });
    flashes.write(Flash {
        at: from,
        color: LinearRgba::rgb(0.3, 0.6, 1.0),
        intensity: 300_000.0,
        secs: 0.35,
    });
}

/// Fly each stasis bolt down the room; where it strikes, a blue burst.
fn fly_bolts(
    mut commands: Commands,
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut StasisBolt, &mut Transform)>,
    mut bursts: MessageWriter<Burst>,
    mut flashes: MessageWriter<Flash>,
    mut noise: MessageWriter<Noise>,
) {
    for (e, mut bolt, mut t) in &mut bolts {
        let step = bolt.velocity * time.delta_secs();
        bolt.left -= step.length();
        if bolt.left > 0.0 {
            t.translation += step;
            continue;
        }
        // Where it struck, and back off the wall.
        let back = -bolt.velocity.normalize();
        let at = t.translation + step - back * bolt.left;
        bursts.write(Burst {
            at,
            dir: back,
            kind: BurstKind::Stasis,
        });
        flashes.write(Flash {
            at: at + back * 0.3,
            color: LinearRgba::rgb(0.3, 0.6, 1.0),
            intensity: 500_000.0,
            secs: 0.6,
        });
        noise.write(Noise("stasisHit", at));
        commands.entity(e).despawn();
    }
}

fn mix(a: LinearRgba, b: LinearRgba, k: f32) -> LinearRgba {
    LinearRgba::rgb(
        a.red + (b.red - a.red) * k,
        a.green + (b.green - a.green) * k,
        a.blue + (b.blue - a.blue) * k,
    )
}
