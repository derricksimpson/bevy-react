//! Particles and flashes, shared by everything that sparks, steams or
//! flares. Each emitter is one mesh of quads animated in its vertex shader
//! (`station_particles.wgsl`, one draw call): the room's dust and its steam
//! vents run forever; bursts — sparks off a hit, a door's steam, a stasis
//! blast — come from a small pool, fired by a [`Burst`] message. One shared
//! light answers [`Flash`] messages (muzzle flashes, impacts), so the deck
//! never holds more than its budget of lights.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// One emitter (see `station_particles.wgsl` for what each field means per
/// kind).
#[derive(Asset, AsBindGroup, Reflect, Clone)]
pub struct FxMaterial {
    /// (kind, amount 0..1, size m, speed).
    #[uniform(0)]
    pub params: Vec4,
    #[uniform(1)]
    pub area: Vec4,
    #[uniform(2)]
    pub wind: Vec4,
    #[uniform(3)]
    pub color: LinearRgba,
    #[uniform(4)]
    pub color2: LinearRgba,
}

impl Material for FxMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/station_particles.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/station_particles.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// What a [`Burst`] throws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BurstKind {
    /// White-hot sparks, falling.
    Sparks,
    /// Blue stasis sparks, slow.
    Stasis,
    /// A cloud of steam.
    Steam,
    /// A breath of frost.
    Frost,
    /// Green motes rising (a medpack).
    Heal,
}

/// Throw a burst at `at`, aimed along `dir` (sparks fan out around it).
#[derive(Message, Clone, Copy)]
pub struct Burst {
    pub at: Vec3,
    pub dir: Vec3,
    pub kind: BurstKind,
}

/// Light the shared flash light at `at` in `color` (linear), `intensity`
/// lumens, fading over `secs`.
#[derive(Message, Clone, Copy)]
pub struct Flash {
    pub at: Vec3,
    pub color: LinearRgba,
    pub intensity: f32,
    pub secs: f32,
}

/// The pooled burst emitters, round robin per family.
#[derive(Resource)]
struct Pool {
    sparks: Vec<(Entity, Handle<FxMaterial>)>,
    clouds: Vec<(Entity, Handle<FxMaterial>)>,
    next: (usize, usize),
}

#[derive(Component)]
struct FlashLight {
    secs: f32,
    peak: f32,
    age: f32,
}

const SPARK_POOL: usize = 6;
const CLOUD_POOL: usize = 4;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<FxMaterial>::default())
        .add_message::<Burst>()
        .add_message::<Flash>()
        .add_systems(Startup, spawn)
        .add_systems(Update, (burst, flash));
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut fx: ResMut<Assets<FxMaterial>>,
) {
    // Dust through the whole room, brightest where the light catches it.
    emitter(
        &mut commands,
        &mut meshes,
        &mut fx,
        Vec3::new(0.0, 2.0, 0.6),
        Quat::IDENTITY,
        FxMaterial {
            params: Vec4::new(0.0, 1.0, 0.009, 1.0),
            area: Vec4::new(20.0, 4.2, 11.5, 0.0),
            wind: Vec4::new(0.03, -0.012, 0.02, 0.0),
            color: LinearRgba::rgb(0.55, 0.6, 0.62),
            color2: LinearRgba::BLACK,
        },
        700,
        1,
    );
    // Steam vents: low by the bulkhead, high on the right wall, under the
    // bench. Each puffs for a moment every few seconds.
    for (at, period, duty, size) in [
        (Vec3::new(-1.45, 0.05, -4.75), 7.0, 0.35, 0.32),
        (Vec3::new(9.7, 3.6, -3.6), 9.5, 0.3, 0.4),
        (Vec3::new(7.6, 0.05, 1.8), 11.0, 0.25, 0.3),
    ] {
        emitter(
            &mut commands,
            &mut meshes,
            &mut fx,
            at,
            Quat::IDENTITY,
            FxMaterial {
                params: Vec4::new(1.0, 1.0, size, 0.45),
                area: Vec4::new(0.12, 1.6, 0.12, period),
                wind: Vec4::new(0.15, 0.3, 0.35, duty),
                color: LinearRgba::new(0.5, 0.56, 0.6, 0.22),
                color2: LinearRgba::new(0.7, 0.75, 0.78, 0.22),
            },
            60,
            2 + at.x as u32,
        );
    }

    let mut pool = Pool {
        sparks: Vec::new(),
        clouds: Vec::new(),
        next: (0, 0),
    };
    for i in 0..SPARK_POOL {
        pool.sparks
            .push(idle(&mut commands, &mut meshes, &mut fx, 70, 20 + i as u32));
    }
    for i in 0..CLOUD_POOL {
        pool.clouds
            .push(idle(&mut commands, &mut meshes, &mut fx, 40, 40 + i as u32));
    }
    commands.insert_resource(pool);

    commands.spawn((
        PointLight {
            intensity: 0.0,
            range: 9.0,
            radius: 0.05,
            ..default()
        },
        Transform::default(),
        FlashLight {
            secs: 1.0,
            peak: 0.0,
            age: 1.0,
        },
    ));
}

/// A burst emitter waiting in the pool (its start time far in the past).
fn idle(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    fx: &mut Assets<FxMaterial>,
    count: u32,
    salt: u32,
) -> (Entity, Handle<FxMaterial>) {
    let material = FxMaterial {
        params: Vec4::new(2.0, 1.0, 0.02, 4.0),
        area: Vec4::new(0.6, 1.0, 0.0, -1000.0),
        wind: Vec4::new(0.0, 0.0, 0.0, 0.5),
        color: LinearRgba::BLACK,
        color2: LinearRgba::BLACK,
    };
    let handle = fx.add(material);
    let e = commands
        .spawn((
            Mesh3d(meshes.add(quads(count, salt))),
            MeshMaterial3d(handle.clone()),
            Transform::default(),
            NoFrustumCulling,
            bevy::light::NotShadowCaster,
        ))
        .id();
    (e, handle)
}

#[allow(clippy::too_many_arguments)]
fn emitter(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    fx: &mut Assets<FxMaterial>,
    at: Vec3,
    rotation: Quat,
    material: FxMaterial,
    count: u32,
    salt: u32,
) {
    commands.spawn((
        Mesh3d(meshes.add(quads(count, salt))),
        MeshMaterial3d(fx.add(material)),
        Transform::from_translation(at).with_rotation(rotation),
        // The quads are placed in the vertex shader: the mesh's bounds mean
        // nothing.
        NoFrustumCulling,
        bevy::light::NotShadowCaster,
    ));
}

/// The look of each burst kind: (kind, size, speed, spread rad, gravity,
/// life s, color, color2).
fn look(kind: BurstKind) -> (f32, f32, f32, f32, f32, f32, LinearRgba, LinearRgba) {
    match kind {
        BurstKind::Sparks => (
            2.0,
            0.009,
            3.4,
            1.0,
            1.0,
            0.7,
            LinearRgba::rgb(9.0, 3.2, 0.6),
            LinearRgba::rgb(14.0, 12.0, 8.0),
        ),
        BurstKind::Stasis => (
            2.0,
            0.02,
            2.4,
            1.4,
            0.15,
            0.9,
            LinearRgba::rgb(0.6, 2.4, 8.0),
            LinearRgba::rgb(5.0, 9.0, 14.0),
        ),
        BurstKind::Heal => (
            2.0,
            0.018,
            0.7,
            1.2,
            -0.25,
            1.1,
            LinearRgba::rgb(0.6, 6.0, 1.6),
            LinearRgba::rgb(5.0, 10.0, 6.0),
        ),
        BurstKind::Steam => (
            3.0,
            0.55,
            1.2,
            0.9,
            -0.1,
            2.2,
            LinearRgba::new(0.62, 0.68, 0.72, 0.35),
            LinearRgba::new(0.85, 0.9, 0.92, 0.35),
        ),
        BurstKind::Frost => (
            3.0,
            0.12,
            0.5,
            1.6,
            0.0,
            1.0,
            LinearRgba::new(0.75, 0.86, 0.95, 0.16),
            LinearRgba::new(0.9, 0.96, 1.0, 0.16),
        ),
    }
}

/// Fire each requested burst from the next pooled emitter of its family.
fn burst(
    mut bursts: MessageReader<Burst>,
    mut pool: ResMut<Pool>,
    time: Res<Time>,
    mut transforms: Query<&mut Transform>,
    mut fx: ResMut<Assets<FxMaterial>>,
) {
    for b in bursts.read() {
        let (kind, size, speed, spread, gravity, life, color, color2) = look(b.kind);
        let cloud = kind == 3.0;
        let pool = &mut *pool;
        let (list, next) = if cloud {
            (&pool.clouds, &mut pool.next.1)
        } else {
            (&pool.sparks, &mut pool.next.0)
        };
        let (entity, handle) = list[*next % list.len()].clone();
        *next += 1;
        if let Ok(mut t) = transforms.get_mut(entity) {
            *t = Transform::from_translation(b.at).with_rotation(Quat::from_rotation_arc(
                Vec3::Y,
                b.dir.normalize_or(Vec3::Y),
            ));
        }
        if let Some(mut m) = fx.get_mut(&handle) {
            m.params = Vec4::new(kind, 1.0, size, speed);
            m.area = Vec4::new(spread, gravity, 0.0, time.elapsed_secs_wrapped());
            m.wind = Vec4::new(0.0, 0.0, 0.0, life);
            m.color = color;
            m.color2 = color2;
        }
    }
}

/// Move the flash light to the latest [`Flash`] and fade it out.
fn flash(
    mut flashes: MessageReader<Flash>,
    time: Res<Time>,
    mut light: Query<(&mut PointLight, &mut Transform, &mut FlashLight)>,
) {
    let Ok((mut point, mut transform, mut state)) = light.single_mut() else {
        return;
    };
    if let Some(f) = flashes.read().last() {
        transform.translation = f.at;
        point.color = f.color.into();
        state.peak = f.intensity;
        state.secs = f.secs.max(0.01);
        state.age = 0.0;
    }
    if state.age >= state.secs {
        if point.intensity != 0.0 {
            point.intensity = 0.0;
        }
        return;
    }
    state.age += time.delta_secs();
    let k = (1.0 - state.age / state.secs).max(0.0);
    point.intensity = state.peak * k * k;
}

/// `count` quads, each with its own random seed in the vertex color.
fn quads(count: u32, salt: u32) -> Mesh {
    let mut rng = 0x2545_f491_u32.wrapping_mul(salt | 1);
    let mut seeds = Vec::with_capacity(count as usize * 4);
    let mut corners = Vec::with_capacity(count as usize * 4);
    let mut indices = Vec::with_capacity(count as usize * 6);
    for q in 0..count {
        let seed = [
            rand(&mut rng),
            rand(&mut rng),
            rand(&mut rng),
            rand(&mut rng),
        ];
        for corner in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            seeds.push(seed);
            corners.push(corner);
        }
        let b = q * 4;
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; corners.len()])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, corners)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, seeds)
    .with_inserted_indices(Indices::U32(indices))
}

/// A xorshift step, in 0..1.
pub fn rand(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state >> 8) as f32 / (1u32 << 24) as f32
}
