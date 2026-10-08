//! What the evaluator's results become: a thumbnail per node (registered as
//! `node-<id>` for the node's preview), the 2D view's image (`view`) and the
//! material maps on the preview's `StandardMaterial`.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_react::RenderTargets;

use super::ViewOutputState;
use super::buf::Buf;
use super::eval::{Evaluator, Slot};
use super::nodes::Ctx;

/// Thumbnail pixels per side: a node preview at 128 logical px, crisp at
/// up to 1x scale.
pub const THUMB: usize = 128;

/// The preview's material and the images its maps live in.
#[derive(Resource)]
pub struct MaterialMaps {
    pub material: Handle<StandardMaterial>,
    base: Handle<Image>,
    metal_rough: Handle<Image>,
    normal: Handle<Image>,
    depth: Handle<Image>,
    emission: Handle<Image>,
    /// The output node key the maps were built from.
    shown: Option<u64>,
}

pub fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut targets: ResMut<RenderTargets>,
) {
    let mut blank = |srgb| images.add(texture(1, &[Vec3::splat(0.5)], srgb, false));
    let maps = MaterialMaps {
        base: blank(true),
        metal_rough: blank(false),
        normal: blank(false),
        depth: blank(false),
        emission: blank(true),
        material: materials.add(StandardMaterial::default()),
        shown: None,
    };
    targets.register("view", images.add(texture(1, &[Vec3::ZERO], true, false)));
    commands.insert_resource(maps);
}

/// An `n × n` texture from display values (`srgb`) or data, optionally with
/// a full box-filtered mip chain and a repeating trilinear sampler (for the
/// material).
fn texture(n: usize, pixels: &[Vec3], srgb: bool, mips: bool) -> Image {
    let mut data = Vec::with_capacity(n * n * 4 * 4 / 3 + 4);
    let encode = |data: &mut Vec<u8>, px: &[Vec3]| {
        for c in px {
            let c = c.clamp(Vec3::ZERO, Vec3::ONE) * 255.0 + 0.5;
            data.extend_from_slice(&[c.x as u8, c.y as u8, c.z as u8, 255]);
        }
    };
    encode(&mut data, pixels);
    let mut levels = 1;
    if mips {
        let (mut level, mut size) = (pixels.to_vec(), n);
        while size > 1 {
            let half = size / 2;
            level = (0..half * half)
                .map(|i| {
                    let (x, y) = (i % half * 2, i / half * 2);
                    (level[y * size + x]
                        + level[y * size + x + 1]
                        + level[(y + 1) * size + x]
                        + level[(y + 1) * size + x + 1])
                        * 0.25
                })
                .collect();
            size = half;
            encode(&mut data, &level);
            levels += 1;
        }
    }
    // `Image::new` checks the data against one level; the chain is longer.
    let mut image = Image::new_uninit(
        Extent3d {
            width: n as u32,
            height: n as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        },
        RenderAssetUsages::default(),
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    if mips {
        // Without COPY_DST Bevy uploads a fresh texture on every change: its
        // in-place reuse path (same descriptor) rewrites mip 0 only, leaving
        // the smaller levels on the previous material.
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING;
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: 16,
            ..default()
        });
    }
    image
}

/// `buf` box-filtered down to `size × size` (or nearest-sampled up).
fn downsample(buf: &Buf, n: usize, size: usize) -> Vec<Vec3> {
    if let Buf::Uniform(c) = buf {
        return vec![*c; size * size];
    }
    let f = (n / size).max(1);
    let norm = 1.0 / (f * f) as f32;
    (0..size * size)
        .map(|i| {
            let (x, y) = (i % size * n / size, i / size * n / size);
            let mut sum = Vec3::ZERO;
            for dy in 0..f {
                for dx in 0..f {
                    sum += buf.color((y + dy) * n + x + dx);
                }
            }
            sum * norm
        })
        .collect()
}

fn pixels(buf: &Buf, n: usize) -> Vec<Vec3> {
    downsample(buf, n, n)
}

/// One thumbnail image per node, rewritten when its node's result changes.
#[derive(Default)]
pub struct Thumbs(HashMap<u32, (Handle<Image>, Option<u64>)>);

pub fn publish_thumbnails(
    evaluator: Res<Evaluator>,
    mut thumbs: Local<Thumbs>,
    mut targets: ResMut<RenderTargets>,
    mut images: ResMut<Assets<Image>>,
) {
    let n = evaluator.resolution();
    thumbs.0.retain(|id, (handle, _)| {
        let live = evaluator.slot(*id).is_some_and(|s| s.def.preview);
        if !live {
            targets.remove(&format!("node-{id}"));
            images.remove(handle.id());
        }
        live
    });
    for slot in evaluator.slots.iter().filter(|s| s.def.preview) {
        let (handle, shown) = thumbs.0.entry(slot.id).or_insert_with(|| {
            let handle = images.add(texture(
                THUMB,
                &vec![Vec3::splat(0.1); THUMB * THUMB],
                true,
                false,
            ));
            targets.register(format!("node-{}", slot.id), handle.clone());
            (handle, None)
        });
        if *shown == Some(slot.key) {
            continue;
        }
        let Some(first) = evaluator
            .outputs(slot)
            .and_then(|o| o.first().cloned().flatten())
        else {
            continue;
        };
        let _ = images.insert(
            handle.id(),
            texture(THUMB, &downsample(&first, n, THUMB), true, false),
        );
        *shown = Some(slot.key);
    }
}

/// What a node's "output" `id` reads: an output of a regular node, or an
/// input of the Material Output (linked, or its inline value).
fn viewed(evaluator: &Evaluator, slot: &Slot, id: &str) -> Option<(std::sync::Arc<Buf>, bool)> {
    if let Some(i) = slot.def.outputs.iter().position(|s| s.id == id) {
        let buf = evaluator.outputs(slot)?.get(i)?.clone()?;
        return Some((buf, true));
    }
    let socket = slot.def.inputs.iter().position(|s| s.id == id)?;
    if slot.inputs[socket].is_some() {
        return Some((evaluator.input(slot, socket)?, true));
    }
    let node = evaluator.graph.nodes.iter().find(|n| n.id == slot.id)?;
    let ctx = Ctx {
        n: 1,
        def: slot.def,
        values: &node.values,
        inputs: &vec![None; slot.def.inputs.len()],
        wanted: 0,
    };
    let c = ctx.input(id).uniform()?;
    Some((std::sync::Arc::new(Buf::Uniform(c)), false))
}

pub fn publish_view(
    evaluator: Res<Evaluator>,
    view: Res<ViewOutputState>,
    mut shown: Local<Option<(u64, String, usize)>>,
    targets: Res<RenderTargets>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(node), Some(output)) = (view.0.node, view.0.output.as_ref()) else {
        return;
    };
    let Some(slot) = evaluator.slot(node) else {
        return;
    };
    let n = evaluator.resolution();
    let current = (slot.key, output.clone(), n);
    if shown.as_ref() == Some(&current) {
        return;
    }
    let Some((buf, _)) = viewed(&evaluator, slot, output) else {
        return;
    };
    if let Some(handle) = targets.get("view") {
        let _ = images.insert(handle.id(), texture(n, &pixels(&buf, n), true, false));
    }
    *shown = Some(current);
}

/// Rebuild the material maps when the Material Output's inputs settle.
pub fn publish_material(
    evaluator: Res<Evaluator>,
    mut maps: ResMut<MaterialMaps>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(slot) = evaluator.slots.iter().find(|s| s.def.kind == "output") else {
        return;
    };
    if maps.shown == Some(slot.key) {
        return;
    }
    let n = evaluator.resolution();
    // Every linked input evaluated, or wait.
    let read = |id: &str| viewed(&evaluator, slot, id);
    let ids = [
        "baseColor",
        "metallic",
        "roughness",
        "normal",
        "height",
        "emission",
    ];
    let inputs: Vec<_> = ids.iter().map(|id| read(id)).collect();
    let linked = |id: &str| {
        let i = slot.def.inputs.iter().position(|s| s.id == id).unwrap();
        slot.inputs[i].is_some()
    };
    if ids
        .iter()
        .zip(&inputs)
        .any(|(id, input)| linked(id) && input.is_none())
    {
        return;
    }
    let buf = |i: usize| inputs[i].as_ref().map(|(b, _)| b.clone());
    let uniform = |c: Vec3| std::sync::Arc::new(Buf::Uniform(c));
    let base = buf(0).unwrap_or_else(|| uniform(Vec3::splat(0.8)));
    let metallic = buf(1).unwrap_or_else(|| uniform(Vec3::ZERO));
    let roughness = buf(2).unwrap_or_else(|| uniform(Vec3::splat(0.5)));
    let normal = buf(3).filter(|_| linked("normal"));
    let height = buf(4).filter(|_| linked("height"));
    let emission = buf(5).unwrap_or_else(|| uniform(Vec3::ZERO));

    let mr: Vec<Vec3> = (0..n * n)
        .map(|i| Vec3::new(0.0, roughness.gray(i), metallic.gray(i)))
        .collect();
    let flat = uniform(Vec3::new(0.5, 0.5, 1.0));
    let normal_px = pixels(normal.as_deref().unwrap_or(&flat), n);
    let mut set = |handle: &Handle<Image>, image: Image| {
        let _ = images.insert(handle.id(), image);
    };
    set(&maps.base, texture(n, &pixels(&base, n), true, true));
    set(&maps.metal_rough, texture(n, &mr, false, true));
    set(&maps.normal, texture(n, &normal_px, false, true));
    set(
        &maps.emission,
        texture(n, &pixels(&emission, n), true, true),
    );
    if let Some(height) = &height {
        // Bevy's depth map runs the other way: 1 is the deepest point.
        let depth: Vec<Vec3> = (0..n * n)
            .map(|i| Vec3::splat(1.0 - height.gray(i)))
            .collect();
        set(&maps.depth, texture(n, &depth, false, true));
    }

    let node = evaluator.graph.nodes.iter().find(|n| n.id == slot.id);
    let value = |id: &str, default: f32| {
        node.and_then(|n| match n.values.get(id) {
            Some(super::Value::Num(v)) => Some(*v),
            _ => None,
        })
        .unwrap_or(default)
    };
    let strength = value("emissionStrength", 1.0);
    let depth_scale = value("heightScale", 4.0) / 100.0;
    if let Some(mut m) = materials.get_mut(&maps.material) {
        m.base_color = Color::WHITE;
        m.base_color_texture = Some(maps.base.clone());
        m.metallic = 1.0;
        m.perceptual_roughness = 1.0;
        m.metallic_roughness_texture = Some(maps.metal_rough.clone());
        m.normal_map_texture = Some(maps.normal.clone());
        m.emissive = LinearRgba::rgb(strength, strength, strength);
        m.emissive_texture = Some(maps.emission.clone());
        m.depth_map = height.is_some().then(|| maps.depth.clone());
        m.parallax_depth_scale = depth_scale;
        m.parallax_mapping_method = bevy::pbr::ParallaxMappingMethod::Relief { max_steps: 4 };
        m.max_parallax_layer_count = 32.0;
    }
    maps.shown = Some(slot.key);
}
