// Filter `holoStatic` (see `examples/deadspace/filters.rs`): the dip
// between two fixtures. The node it sits on is empty; the pass writes a
// screen of hologram static over the frame — tinted noise, dark scan rows,
// bright tear lines — and React fades the node in and out.
//
// Params (declaration-order packing of `HoloStatic`):
//   params[0]  tint (straight linear RGBA; alpha = coverage)
//
// PREMULTIPLY: writes opaque-ish color (rgb * a, a), valid by construction.

#import bevy_react::filter::{FullscreenVertexOutput, uniforms}

fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn unit(a: u32, b: u32) -> f32 {
    return f32(pcg(a ^ pcg(b))) / 4294967296.0;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let tint = uniforms.params[0];
    let res = uniforms.resolution;
    let step = u32(uniforms.time * 30.0);
    // Coarse noise cells (2 px), rows of them sliding at their own speed.
    let cell = vec2<u32>(in.uv * res / 2.0);
    let n = unit(cell.x + cell.y * 7001u, step);
    let row = unit(cell.y, step ^ 0x9e37u);
    let scan = select(1.0, 0.35, (cell.y & 1u) == 0u);
    let tear = select(0.0, 1.0, row > 0.985);
    let v = (n * n * n * 0.7 + tear * 1.2) * scan;
    let rgb = mix(vec3<f32>(v * 0.2), tint.rgb * v * 1.5, 0.85);
    let a = clamp(tint.a, 0.0, 1.0);
    return vec4<f32>(rgb * a, a);
}
