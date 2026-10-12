// Procedural wear over the deck's lit materials (see
// `examples/deadspace/station/kit.rs`): an `ExtendedMaterial` that edits the
// PBR input — base color, roughness — before Bevy's own lighting and fog.
// `mode` = (pattern, seed, scale m, grime 0..1); `tint` = stripe paint.
//
//   0 steel   panels `scale` m across keyed to world space (the plane is
//             picked by the surface's facing), dark seams, each panel its
//             own shade, grime pooling low and running down from the seams.
//   1 grate   floor grating: rows of dark slots between bars, panel
//             borders, the walkway scuffed bright down the middle.
//   2 hazard  diagonal warning stripes `scale` m wide in `tint`, worn
//             through to the steel where feet and hands go.
//   3 worn    scuffs and scratches over the UVs: suit plates, weapons.

import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> mode: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<uniform> tint: vec4<f32>;

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash12(i), hash12(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash12(i + vec2<f32>(0.0, 1.0)), hash12(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(p: vec2<f32>) -> f32 {
    return 0.5 * vnoise(p) + 0.25 * vnoise(p * 2.03 + 17.0) + 0.125 * vnoise(p * 4.01 + 31.0);
}

/// The 2D coordinates of a world point on the plane the surface faces, with
/// "down" along -y where the surface is a wall.
fn plane_uv(p: vec3<f32>, n: vec3<f32>) -> vec2<f32> {
    let a = abs(n);
    if a.y > a.x && a.y > a.z {
        return p.xz;
    }
    if a.x > a.z {
        return vec2<f32>(p.z, p.y);
    }
    return vec2<f32>(p.x, p.y);
}

/// 0 on a seam, 1 a few cm away.
fn seams(uv: vec2<f32>, cell: vec2<f32>, width: f32) -> f32 {
    let f = abs(fract(uv / cell) - 0.5) * cell;
    let d = min(cell.x * 0.5 - f.x, cell.y * 0.5 - f.y);
    return smoothstep(0.0, width, d);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

    let pos = in.world_position.xyz;
    let n = pbr_input.world_normal;
    let seed = mode.y;
    let scale = max(mode.z, 0.01);
    let grime = mode.w;
    var base = pbr_input.material.base_color.rgb;
    var rough = pbr_input.material.perceptual_roughness;
    let uv2 = plane_uv(pos, n) + seed * 13.1;

    switch u32(mode.x + 0.5) {
        case 0u: {
            // Panels twice as tall as wide, each its own shade.
            let cell = vec2<f32>(scale, scale * 1.6);
            let id = floor(uv2 / cell);
            base *= 0.82 + 0.3 * hash12(id + seed);
            let s = seams(uv2, cell, 0.018);
            base *= mix(0.25, 1.0, s);
            // Rust and soot run down from each seam and pool near the floor.
            let run = fbm(vec2<f32>(uv2.x * 9.0, uv2.y * 0.7)) * (1.0 - fract(uv2.y / cell.y));
            let low = 1.0 - smoothstep(0.0, 1.4, pos.y);
            let dirt = clamp(fbm(uv2 * 1.7) * 1.2 - 0.35 + low * 0.5 + run * 0.35, 0.0, 1.0) * grime;
            base = mix(base, base * vec3<f32>(0.42, 0.36, 0.3), dirt);
            rough = clamp(rough + dirt * 0.3 - (1.0 - s) * 0.1, 0.05, 1.0);
        }
        case 1u: {
            let tile = fract(uv2 / scale);
            let border = smoothstep(0.0, 0.03, min(min(tile.x, 1.0 - tile.x), min(tile.y, 1.0 - tile.y)));
            // Slots: 3 cm bars, 1.5 cm gaps, across the tile.
            let bar = fract(uv2.x * 22.0);
            let slot = smoothstep(0.62, 0.68, bar) * (1.0 - smoothstep(0.92, 0.98, bar));
            let in_tile = smoothstep(0.06, 0.08, tile.y) * (1.0 - smoothstep(0.92, 0.94, tile.y));
            let hole = slot * in_tile;
            base *= mix(1.0, 0.08, hole) * mix(0.35, 1.0, border);
            // The walkway down the middle is worn bright, the edges filthy.
            let walk = 1.0 - smoothstep(1.0, 3.5, abs(pos.x));
            let dirt = clamp(fbm(uv2 * 1.3) * 1.3 - 0.4 + (1.0 - walk) * 0.25, 0.0, 1.0) * grime;
            base = mix(base, base * vec3<f32>(0.4, 0.36, 0.32), dirt);
            base *= 1.0 + walk * 0.25 * (1.0 - hole);
            rough = clamp(rough + dirt * 0.25 + hole * 0.4 - walk * 0.15, 0.05, 1.0);
        }
        case 2u: {
            let d = (uv2.x + uv2.y) / scale;
            let stripe = smoothstep(0.45, 0.5, abs(fract(d) - 0.5) * 2.0);
            let wear = smoothstep(0.55, 0.75, fbm(uv2 * 6.0 + seed));
            let paint = mix(tint.rgb, base, wear);
            base = mix(base, paint, stripe);
            rough = mix(rough, 0.75, stripe * (1.0 - wear));
        }
        default: {
            var uv = vec2<f32>(0.5);
@if(VERTEX_UVS_A)
            uv = in.uv;
            let scratch = smoothstep(0.86, 0.97, vnoise(vec2<f32>(uv.x * 46.0, uv.y * 3.5) + seed));
            let scuff = fbm(uv * 5.0 + seed);
            base *= 0.9 + 0.2 * scuff;
            base = mix(base, vec3<f32>(0.5, 0.5, 0.47), scratch * 0.35 * grime);
            rough = clamp(rough - scratch * 0.25 + (scuff - 0.5) * 0.12, 0.05, 1.0);
        }
    }
    pbr_input.material.base_color = vec4<f32>(base, pbr_input.material.base_color.a);
    pbr_input.material.perceptual_roughness = rough;

    // `@if` scopes one statement: a block would scope `out` too.
@if(PREPASS_PIPELINE)
    let out = deferred_output(in, pbr_input);
@if(!PREPASS_PIPELINE)
    var out: FragmentOutput;
@if(!PREPASS_PIPELINE)
    out.color = apply_pbr_lighting(pbr_input);
@if(!PREPASS_PIPELINE)
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
