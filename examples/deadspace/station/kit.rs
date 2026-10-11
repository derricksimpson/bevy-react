//! What the deck is made of: one palette of materials every builder shares
//! (`Kit`), the `Paint` extension that grimes them (`station_paint.wgsl`),
//! and `Build`, which merges a builder's boxes and pipes into one mesh per
//! material — the whole room is a few dozen draw calls, not thousands.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type PaintMaterial = ExtendedMaterial<StandardMaterial, Paint>;

/// Procedural wear over a lit `StandardMaterial` (see `station_paint.wgsl`):
/// `mode` = (pattern, seed, scale m, grime 0..1); `tint` = the stripes'
/// paint for `Pattern::Hazard`.
#[derive(Asset, AsBindGroup, Reflect, Clone, Default)]
pub struct Paint {
    #[uniform(100)]
    pub mode: Vec4,
    #[uniform(101)]
    pub tint: LinearRgba,
}

impl MaterialExtension for Paint {
    fn fragment_shader() -> ShaderRef {
        "shaders/station_paint.wgsl".into()
    }
}

/// The paint patterns (the shader's `mode.x`).
#[derive(Clone, Copy)]
pub enum Pattern {
    /// Panels with seams, keyed to world space; grime toward the floor.
    Steel = 0,
    /// Floor grating: slots between bars, scuffed walkways.
    Grate = 1,
    /// Diagonal warning stripes in `tint`, worn through.
    Hazard = 2,
    /// Scuffs and scratches over the UVs (suit plates, weapons).
    Worn = 3,
}

/// The deck's shared materials. Every builder takes from here so the room,
/// the fixtures, the suit and the weapons read as one place.
#[derive(Resource)]
pub struct Kit {
    pub steel: Handle<PaintMaterial>,
    pub dark: Handle<PaintMaterial>,
    pub floor: Handle<PaintMaterial>,
    pub hazard: Handle<PaintMaterial>,
    pub rust: Handle<PaintMaterial>,
    pub rubber: Handle<StandardMaterial>,
    pub pipe: Handle<StandardMaterial>,
    pub screen: Handle<StandardMaterial>,
    pub cyan: Handle<StandardMaterial>,
    pub cyan_dim: Handle<StandardMaterial>,
    pub amber: Handle<StandardMaterial>,
    pub red: Handle<StandardMaterial>,
    pub red_dim: Handle<StandardMaterial>,
    pub amber_dim: Handle<StandardMaterial>,
    pub white: Handle<StandardMaterial>,
    pub warm: Handle<StandardMaterial>,
}

impl FromWorld for Kit {
    fn from_world(world: &mut World) -> Self {
        let mut paints = world.resource_mut::<Assets<PaintMaterial>>();
        let steel = paints.add(paint(
            Srgba::hex("4a525a").unwrap(),
            0.8,
            0.5,
            Pattern::Steel,
            1.2,
            0.75,
        ));
        let dark = paints.add(paint(
            Srgba::hex("262c31").unwrap(),
            0.75,
            0.55,
            Pattern::Steel,
            0.8,
            0.6,
        ));
        let floor = paints.add(paint(
            Srgba::hex("3a4146").unwrap(),
            0.85,
            0.45,
            Pattern::Grate,
            1.0,
            0.8,
        ));
        let mut hazard = paint(
            Srgba::hex("191b1c").unwrap(),
            0.3,
            0.6,
            Pattern::Hazard,
            0.16,
            0.7,
        );
        hazard.extension.tint = Srgba::hex("c99a1e").unwrap().into();
        let hazard = paints.add(hazard);
        let rust = paints.add(paint(
            Srgba::hex("5a3a26").unwrap(),
            0.55,
            0.62,
            Pattern::Worn,
            1.0,
            0.6,
        ));

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let solid = |m: &mut Assets<StandardMaterial>, color: &str, rough: f32, metal: f32| {
            m.add(StandardMaterial {
                base_color: Srgba::hex(color).unwrap().into(),
                perceptual_roughness: rough,
                metallic: metal,
                ..default()
            })
        };
        let rubber = solid(&mut materials, "0c0e10", 0.9, 0.0);
        let pipe = solid(&mut materials, "5d666d", 0.4, 0.9);
        let screen = solid(&mut materials, "05090c", 0.25, 0.2);
        Self {
            steel,
            dark,
            floor,
            hazard,
            rust,
            rubber,
            pipe,
            screen,
            cyan: glow(&mut materials, LinearRgba::rgb(0.5, 2.6, 3.6)),
            cyan_dim: glow(&mut materials, LinearRgba::rgb(0.05, 0.35, 0.5)),
            amber: glow(&mut materials, LinearRgba::rgb(5.0, 1.9, 0.35)),
            red: glow(&mut materials, LinearRgba::rgb(4.5, 0.25, 0.12)),
            red_dim: glow(&mut materials, LinearRgba::rgb(0.9, 0.05, 0.03)),
            amber_dim: glow(&mut materials, LinearRgba::rgb(1.1, 0.42, 0.08)),
            white: glow(&mut materials, LinearRgba::rgb(3.4, 3.8, 4.0)),
            warm: glow(&mut materials, LinearRgba::rgb(7.0, 4.6, 2.4)),
        }
    }
}

fn paint(
    color: Srgba,
    metallic: f32,
    roughness: f32,
    pattern: Pattern,
    scale: f32,
    grime: f32,
) -> PaintMaterial {
    PaintMaterial {
        base: StandardMaterial {
            base_color: color.into(),
            metallic,
            perceptual_roughness: roughness,
            ..default()
        },
        extension: Paint {
            mode: Vec4::new(pattern as u8 as f32, 0.0, scale, grime),
            tint: LinearRgba::BLACK,
        },
    }
}

/// Light itself: an emissive surface (`color` linear; above 1 it blooms).
pub fn glow(
    materials: &mut Assets<StandardMaterial>,
    color: LinearRgba,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: color,
        ..default()
    })
}

/// Additive light with no surface (beams, flashes, haze cones): unlit,
/// `color` linear HDR.
pub fn additive(
    materials: &mut Assets<StandardMaterial>,
    color: LinearRgba,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color.into(),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    })
}

/// Merges everything one builder adds into one mesh per material.
pub struct Build<M: Material> {
    parts: Vec<(Handle<M>, Mesh)>,
}

impl<M: Material> Default for Build<M> {
    fn default() -> Self {
        Self { parts: Vec::new() }
    }
}

impl<M: Material> Build<M> {
    /// Add `mesh`, placed by `at`, in `material`.
    pub fn add(&mut self, material: &Handle<M>, mesh: impl Into<Mesh>, at: Transform) {
        let mesh = mesh.into().transformed_by(at);
        match self.parts.iter_mut().find(|(m, _)| m == material) {
            Some((_, merged)) => merged.merge(&mesh).expect("primitive meshes merge"),
            None => self.parts.push((material.clone(), mesh)),
        }
    }

    /// A box `size` centered at `at`.
    pub fn cube(&mut self, material: &Handle<M>, size: Vec3, at: Vec3) {
        self.add(
            material,
            Cuboid::from_size(size),
            Transform::from_translation(at),
        );
    }

    /// A cylinder of `radius` from `a` to `b`.
    pub fn rod(&mut self, material: &Handle<M>, radius: f32, a: Vec3, b: Vec3) {
        let d = b - a;
        let mesh = Cylinder::new(radius, d.length()).mesh().resolution(12);
        let at = Transform::from_translation((a + b) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize()));
        self.add(material, mesh, at);
    }

    /// Spawn one entity per material, each with `extra`, under `parent`.
    pub fn spawn(
        self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        parent: Option<Entity>,
        extra: impl Bundle + Clone,
    ) {
        for (material, mesh) in self.parts {
            let mut e = commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                extra.clone(),
            ));
            if let Some(parent) = parent {
                e.insert(ChildOf(parent));
            }
        }
    }
}
