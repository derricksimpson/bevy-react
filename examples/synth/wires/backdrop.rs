//! The backdrop: one full-screen quad the camera films under the UI, drawn
//! by `assets/shaders/backdrop.wesl` — the dotted grid and every wire. The
//! camera is HDR with bloom, so the signals running along the wires glow,
//! and the floating panels' `backdropFilter` frosts what is beneath them.

use bevy::camera::Hdr;
use bevy::ecs::system::SystemParam;
use bevy::math::Affine2;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::{Cable, FLASH, MAX_WIRES, Signal};

/// The one camera: the backdrop and the UI.
#[derive(Component)]
pub struct MainCamera;

pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<Backdrop>::default())
            .add_systems(Startup, spawn);
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct Backdrop {
    #[uniform(0)]
    frame: Frame,
    #[uniform(1)]
    wires: [WireUniform; MAX_WIRES],
}

impl Material for Backdrop {
    fn fragment_shader() -> ShaderRef {
        "shaders/backdrop.wesl".into()
    }
}

/// Mirrors `struct Frame` in the shader.
#[derive(ShaderType, Clone, Copy, Default, PartialEq, Debug)]
pub struct Frame {
    /// Pan (xy, physical px), physical px per graph unit, physical px per
    /// logical px.
    pub view: Vec4,
    /// Pointer (xy, physical px), wire count, play (0..1).
    pub pointer: Vec4,
    /// Flow clock (s), eighth-note steps.
    pub time: Vec4,
}

/// Mirrors `struct Wire` in the shader.
#[derive(ShaderType, Clone, Copy, Default, PartialEq, Debug)]
pub struct WireUniform {
    /// Output end, first control point (physical px).
    a: Vec4,
    /// Second control point, input end.
    b: Vec4,
    /// Linear rgb, opacity.
    color: Vec4,
    /// Half width (px), signal, glow radius (px), drawn from.
    style: Vec4,
    /// Drawn to, flash (0..1, negative: none), hot (0..1), growing.
    anim: Vec4,
    /// What it can light: min xy, max xy.
    bbox: Vec4,
}

#[derive(Resource)]
struct BackdropMaterial(Handle<Backdrop>);

/// Packs a cable for the shader, in physical px.
pub fn wire(
    cable: &Cable,
    signal: Signal,
    color: LinearRgba,
    opacity: f32,
    to_screen: &Affine2,
) -> WireUniform {
    let [p0, p1, p2, p3] = [cable.ends[0], cable.ctrl[0], cable.ctrl[1], cable.ends[1]]
        .map(|p| to_screen.transform_point2(p));
    // Physical px per graph unit.
    let unit = to_screen.matrix2.x_axis.x;
    let half = (1.5 * unit).max(0.8);
    let glow = (7.0 * unit).max(3.0);
    let margin = half * 1.4 + glow * 3.0;
    let (min, max) = [p0, p1, p2, p3]
        .iter()
        .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        });
    let signal = match signal {
        Signal::Audio => 0.0,
        Signal::Cv => 1.0,
        Signal::Gate => 2.0,
    };
    WireUniform {
        a: Vec4::new(p0.x, p0.y, p1.x, p1.y),
        b: Vec4::new(p2.x, p2.y, p3.x, p3.y),
        color: Vec4::new(color.red, color.green, color.blue, opacity),
        style: Vec4::new(half, signal, glow, cable.span[0]),
        anim: Vec4::new(
            cable.span[1],
            cable.flash.map_or(-1.0, |t| t / FLASH),
            cable.hot,
            cable.grow.is_some() as u8 as f32,
        ),
        bbox: Vec4::new(
            min.x - margin,
            min.y - margin,
            max.x + margin,
            max.y + margin,
        ),
    }
}

/// Writes a frame's uniforms — only when they changed, so an idle patch
/// re-uploads nothing.
#[derive(SystemParam)]
pub struct Writer<'w> {
    material: Res<'w, BackdropMaterial>,
    materials: ResMut<'w, Assets<Backdrop>>,
}

impl Writer<'_> {
    pub fn write(&mut self, frame: Frame, wires: &[WireUniform]) {
        let mut next = [WireUniform::default(); MAX_WIRES];
        next[..wires.len()].copy_from_slice(wires);
        let handle = &self.material.0;
        if self
            .materials
            .get(handle)
            .is_some_and(|m| m.frame != frame || m.wires != next)
            && let Some(mut m) = self.materials.get_mut(handle)
        {
            m.frame = frame;
            m.wires = next;
        }
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Backdrop>>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(8, 10, 16)),
            ..default()
        },
        // One unit per logical px; the quad is far bigger than any window.
        Projection::Orthographic(OrthographicProjection::default_3d()),
        Hdr,
        Msaa::Off,
        Bloom {
            intensity: 0.22,
            ..Bloom::NATURAL
        },
        IsDefaultUiCamera,
        MainCamera,
    ));
    let material = materials.add(Backdrop {
        frame: Frame::default(),
        wires: [WireUniform::default(); MAX_WIRES],
    });
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(1.0e5, 1.0e5))),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(0.0, 0.0, -100.0),
    ));
    commands.insert_resource(BackdropMaterial(material));
}
