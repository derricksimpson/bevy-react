//! The 3D preview: the graph's material on a mesh, lit by a procedural studio
//! (a cubemap filtered into reflections where compute shaders exist — not on
//! WebGL2, where the key and fill lights carry it), rendered by its own
//! camera into the `preview` target a `<portal>` shows.

use bevy::asset::RenderAssetUsages;
use bevy::camera::Hdr;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::math::Affine2;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy_react::{
    PortalCamera, ReactAppExt, RenderMode, RenderTargetSpec, RenderTargets, Resolution,
    react_message,
};

use crate::graph::MaterialMaps;

/// The preview world's render layer (the UI camera sees layer 0 only).
const LAYER: usize = 1;

/// React → Bevy: which mesh, and whether it turns by itself.
#[react_message(name = "preview.set")]
pub struct SetPreview {
    /// `sphere`, `cube`, `plane` or `torus`.
    pub mesh: String,
    pub spin: bool,
}

/// React → Bevy: a drag over the preview orbits the camera (logical px), the
/// wheel zooms (positive = out).
#[react_message(name = "preview.orbit")]
pub struct Orbit {
    pub dx: f32,
    pub dy: f32,
    pub zoom: f32,
}

#[derive(Resource)]
struct Preview {
    meshes: Vec<(&'static str, Handle<Mesh>, Vec2)>,
    spin: bool,
    yaw: f32,
    pitch: f32,
    distance: f32,
}

#[derive(Component)]
struct Subject;

#[derive(Component)]
struct PreviewCamera;

pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        register_bindings(app);
        app.add_systems(Startup, setup.after(crate::graph::setup_textures))
            .add_systems(Update, (turn, place_camera));
    }
}

pub fn register_bindings(app: &mut App) {
    app.add_react_handler(on_set).add_react_handler(on_orbit);
}

fn mesh(mesh: impl Into<Mesh>) -> Mesh {
    mesh.into()
        .with_generated_tangents()
        .expect("primitive meshes have uvs and normals")
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut targets: ResMut<RenderTargets>,
    maps: Res<MaterialMaps>,
) {
    let layer = RenderLayers::layer(LAYER);
    // Each mesh with the UV scale that keeps texels square on it.
    let list = vec![
        (
            "sphere",
            meshes.add(mesh(Sphere::new(1.0).mesh().uv(96, 48))),
            Vec2::new(2.0, 1.0),
        ),
        (
            "cube",
            meshes.add(mesh(Cuboid::from_length(1.5))),
            Vec2::ONE,
        ),
        (
            "plane",
            meshes.add(mesh(
                Plane3d::new(Vec3::Z, Vec2::splat(1.2))
                    .mesh()
                    .subdivisions(1),
            )),
            Vec2::splat(1.0),
        ),
        (
            "torus",
            meshes.add(mesh(
                Torus::new(0.5, 1.05)
                    .mesh()
                    .minor_resolution(48)
                    .major_resolution(96),
            )),
            Vec2::new(4.0, 1.0),
        ),
    ];
    commands.spawn((
        Mesh3d(list[0].1.clone()),
        MeshMaterial3d(maps.material.clone()),
        Transform::default(),
        Subject,
        layer.clone(),
    ));
    commands.insert_resource(Preview {
        meshes: list,
        spin: true,
        yaw: 0.6,
        pitch: 0.25,
        distance: 4.2,
    });

    let target = targets.create(
        &mut images,
        "preview",
        RenderTargetSpec {
            size: Resolution::Auto,
            mode: RenderMode::Live,
            ..default()
        },
    );
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.16, 0.16, 0.17)),
            ..default()
        },
        Projection::from(PerspectiveProjection {
            fov: 30f32.to_radians(),
            ..default()
        }),
        Hdr,
        Bloom::NATURAL,
        Tonemapping::AgX,
        GeneratedEnvironmentMapLight {
            environment_map: images.add(studio()),
            intensity: 900.0,
            ..default()
        },
        target.camera_target(),
        PortalCamera("preview".into()),
        PreviewCamera,
        Transform::default(),
        layer.clone(),
    ));
    // Key and fill, so the WebGL2 build (no filtered environment) still
    // reads the surface.
    commands.spawn((
        DirectionalLight {
            illuminance: 4000.0,
            ..default()
        },
        Transform::from_xyz(-3.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 1200.0,
            color: Color::srgb(0.75, 0.82, 1.0),
            ..default()
        },
        Transform::from_xyz(4.0, 1.0, -2.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer,
    ));
}

/// A photo studio as an HDR cubemap: a gray sweep, a big softbox up front
/// left, a strip light on the right and a rim light behind.
fn studio() -> Image {
    const SIZE: usize = 128;
    let face_dir = |face: usize, u: f32, v: f32| match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    };
    // A rectangular light: brightness where `dir` points into it.
    let softbox = |dir: Vec3, center: Vec3, half: Vec2, power: f32| {
        let center = center.normalize();
        let right = center.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = right.cross(center);
        let along = dir.dot(center);
        if along <= 0.0 {
            return 0.0;
        }
        let p = dir / along - center;
        let (x, y) = (p.dot(right) / half.x, p.dot(up) / half.y);
        let edge = (1.0 - x.abs().max(y.abs())).clamp(0.0, 0.15) / 0.15;
        edge * power
    };
    let mut data = Vec::with_capacity(SIZE * SIZE * 6 * 8);
    for face in 0..6 {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let u = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                let d = face_dir(face, u, v).normalize();
                let sweep = 0.05 + 0.1 * (d.y * 0.5 + 0.5).powf(1.5);
                let light = softbox(d, Vec3::new(-1.0, 0.9, 1.2), Vec2::new(0.55, 0.4), 9.0)
                    + softbox(d, Vec3::new(1.6, 0.2, 0.3), Vec2::new(0.12, 0.7), 5.0)
                    + softbox(d, Vec3::new(0.2, 0.5, -1.4), Vec2::new(0.6, 0.15), 4.0);
                let c = Vec3::splat(sweep) + Vec3::new(1.0, 0.97, 0.92) * light;
                for channel in [c.x, c.y, c.z, 1.0] {
                    data.extend_from_slice(&f16_bits(channel).to_le_bytes());
                }
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

/// An `f32` as IEEE half-float bits (positive, normal range: the studio's
/// values).
fn f16_bits(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    if exp <= 0 {
        return sign;
    }
    if exp >= 31 {
        return sign | 0x7c00;
    }
    sign | ((exp as u16) << 10) | ((bits & 0x7f_ffff) >> 13) as u16
}

fn on_set(
    set: On<SetPreview>,
    mut preview: ResMut<Preview>,
    mut subject: Single<&mut Mesh3d, With<Subject>>,
    maps: Res<MaterialMaps>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    preview.spin = set.spin;
    let Some((_, handle, uv)) = preview.meshes.iter().find(|(name, ..)| *name == set.mesh) else {
        return;
    };
    if subject.0 != *handle {
        subject.0 = handle.clone();
    }
    let uv_transform = Affine2::from_scale(*uv);
    if materials
        .get(&maps.material)
        .is_some_and(|m| m.uv_transform != uv_transform)
        && let Some(mut m) = materials.get_mut(&maps.material)
    {
        m.uv_transform = uv_transform;
    }
}

fn on_orbit(orbit: On<Orbit>, mut preview: ResMut<Preview>) {
    preview.yaw -= orbit.dx * 0.01;
    preview.pitch = (preview.pitch + orbit.dy * 0.01).clamp(-1.3, 1.3);
    preview.distance = (preview.distance * (1.0 + orbit.zoom * 0.1)).clamp(2.2, 9.0);
}

fn turn(
    time: Res<Time>,
    preview: Res<Preview>,
    mut subject: Single<&mut Transform, With<Subject>>,
) {
    if preview.spin {
        subject.rotate_y(time.delta_secs() * 0.4);
    }
}

fn place_camera(
    preview: Res<Preview>,
    mut camera: Single<&mut Transform, (With<PreviewCamera>, Without<Subject>)>,
) {
    if !preview.is_changed() {
        return;
    }
    let dir = Quat::from_euler(EulerRot::YXZ, preview.yaw, -preview.pitch, 0.0) * Vec3::Z;
    **camera = Transform::from_translation(dir * preview.distance).looking_at(Vec3::ZERO, Vec3::Y);
}
