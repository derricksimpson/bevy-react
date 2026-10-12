// A link between two sockets: a cubic bezier with a dark rim, its color
// running from the start socket's type to the end socket's. Geometry is in
// the box's logical pixels; screen-space derivatives give a one-pixel
// antialiased edge at any zoom.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Wire {
    // The box's logical size (xy).
    size: vec4<f32>,
    // Start (xy), first control point (zw).
    a: vec4<f32>,
    // Second control point (xy), end (zw).
    b: vec4<f32>,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    // Thickness, dash period (0 = solid).
    stroke: vec4<f32>,
};

@group(1) @binding(0) var<uniform> wire: Wire;

const SEGMENTS: i32 = 32;

fn bezier(t: f32) -> vec2<f32> {
    let s = 1.0 - t;
    return s * s * s * wire.a.xy + 3.0 * s * s * t * wire.a.zw + 3.0 * s * t * t * wire.b.xy
        + t * t * t * wire.b.zw;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * wire.size.xy;
    // The nearest point on the curve, as a polyline: its distance, its curve
    // parameter (for the color) and its arc length (for the dashes).
    var best = 1e9;
    var best_t = 0.0;
    var best_s = 0.0;
    var walked = 0.0;
    var prev = bezier(0.0);
    for (var i = 1; i <= SEGMENTS; i++) {
        let cur = bezier(f32(i) / f32(SEGMENTS));
        let seg = cur - prev;
        let h = clamp(dot(p - prev, seg) / max(dot(seg, seg), 1e-6), 0.0, 1.0);
        let d = length(p - prev - seg * h);
        if d < best {
            best = d;
            best_t = (f32(i - 1) + h) / f32(SEGMENTS);
            best_s = walked + length(seg) * h;
        }
        walked += length(seg);
        prev = cur;
    }
    let aa = max(fwidth(best), 1e-3);
    let half = wire.stroke.x * 0.5;
    var core = 1.0 - smoothstep(half - aa, half, best);
    let dashed = wire.stroke.y > 0.0;
    if dashed && fract(best_s / wire.stroke.y) > 0.5 {
        core = 0.0;
    }
    let color = mix(wire.color_start, wire.color_end, best_t);
    // Solid wires sit in a one-pixel dark rim; dashed ones (the knife, a
    // muted link) don't.
    let rim = 1.0 - smoothstep(half + 1.0 - aa, half + 1.0, best);
    let rgb = mix(vec3<f32>(0.02, 0.02, 0.02), color.rgb, core);
    return vec4<f32>(rgb, select(rim, core, dashed) * color.a);
}
