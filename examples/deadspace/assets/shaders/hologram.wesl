// The hologram material (see `examples/deadspace/holo/material.rs`): a React
// `<surface>` drawn as projected light. The UI arrives premultiplied and
// linear and leaves the same way, its colors raised by `gain` so the bright
// parts bloom; dark translucent plates keep their alpha and dim the room
// behind them.
//
//   * Unfolding (`presence` 0 → 1): a line of light grows out from the
//     center, then the image opens up and down out of it, squeezed flat at
//     first, flickering until it settles. Folding runs it backwards.
//   * Scanlines drift down every 3 logical px; the light shimmers and now
//     and then drops out for a frame.
//   * From behind, the image shows mirrored and dimmed.
//
// Uniforms:
//   look  = (presence 0..1, gain, seed, flicker 0..1)
//   shape = (logical px w, logical px h, scanline strength 0..1, -)

import bevy_pbr::render::{forward_io::VertexOutput, mesh_view_bindings::globals};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> look: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> shape: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var ui: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var ui_sampler: sampler;

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let presence = look.x;
    let t = globals.time;
    var uv = in.uv;
    if !front {
        uv.x = 1.0 - uv.x;
    }

    // The line, then the opening.
    let grow = smoothstep(0.0, 0.35, presence);
    let open = smoothstep(0.3, 1.0, presence);
    let dx = abs(uv.x - 0.5);
    let dy = abs(uv.y - 0.5);
    let half_h = max(open * 0.5, 0.5 / shape.y);
    let in_x = step(dx, grow * 0.5);
    let inside = in_x * step(dy, half_h);
    // Squeezed into the opening band, the image unfolds rather than being
    // uncovered.
    let suv = vec2<f32>(uv.x, 0.5 + (uv.y - 0.5) * 0.5 / half_h);
    var c = textureSampleLevel(ui, ui_sampler, suv, 0.0);

    // The forming edge: a bright seam riding the opening's top and bottom
    // (2 px wide), fading out as the image settles.
    let seam = in_x * (1.0 - smoothstep(0.0, 2.0 / shape.y, abs(dy - half_h))) * (1.0 - open);

    // Scanlines, drifting down.
    let row = uv.y * shape.y - t * 9.0;
    let scan = 1.0 - shape.z * (0.5 + 0.5 * sin(row * 2.0944));

    // Shimmer, the odd dropped frame, and a stutter while forming.
    let frame = floor(t * 24.0);
    let n = hash(vec2<f32>(frame, look.z));
    let shimmer = 1.0 - look.w * (0.5 + 0.5 * sin(t * 7.3 + look.z * 3.1));
    let drop = select(1.0, 0.55, n < 0.02);
    let stutter = mix(select(1.0, 0.35, n < 0.45), 1.0, open);
    var gain = look.y * scan * shimmer * drop * stutter;
    if !front {
        gain *= 0.45;
    }

    c = vec4<f32>(c.rgb * gain, c.a) * inside;
    let light = vec3<f32>(0.55, 0.92, 1.0) * seam * 3.0;
    let out = vec4<f32>(c.rgb + light, c.a);
    if out.a < 0.002 && max(out.r, max(out.g, out.b)) < 0.002 {
        discard;
    }
    return out;
}
