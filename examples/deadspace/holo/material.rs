//! The hologram material: `hologram.wgsl` over a React `<surface>` texture.
//! Premultiplied blending (`One`, `OneMinusSrcAlpha`), so the UI's dark
//! translucent plates dim the world behind and its lines add light; unlit,
//! both sides, no shadows.

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct HologramMaterial {
    /// (presence 0..1, gain, seed, flicker 0..1)
    #[uniform(0)]
    pub look: Vec4,
    /// (texture logical px w, h, scanline strength, edge glow)
    #[uniform(1)]
    pub shape: Vec4,
    #[texture(2)]
    #[sampler(3)]
    pub ui: Handle<Image>,
}

impl HologramMaterial {
    pub fn new(ui: Handle<Image>, px: Vec2, seed: f32) -> Self {
        Self {
            look: Vec4::new(0.0, 1.35, seed, 0.06),
            shape: Vec4::new(px.x, px.y, 0.12, 1.0),
            ui,
        }
    }
}

impl Material for HologramMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/hologram.wgsl".into()
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

    // A hologram seen from behind is still light (the shader mirrors it).
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}
