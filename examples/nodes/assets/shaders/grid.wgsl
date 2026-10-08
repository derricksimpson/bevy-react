// The graph's backdrop: a dot lattice that pans and zooms with the view.
// Minor dots fade out as they crowd together; every fifth dot stays.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Grid {
    // The box's logical size (xy).
    size: vec4<f32>,
    // Pan offset (xy), spacing (z), all logical pixels.
    params: vec4<f32>,
    color: vec4<f32>,
};

@group(1) @binding(0) var<uniform> grid: Grid;

fn dots(p: vec2<f32>, spacing: f32, radius: f32) -> f32 {
    let q = (fract(p / spacing + 0.5) - 0.5) * spacing;
    let d = length(q);
    let aa = max(fwidth(d), 1e-3);
    return 1.0 - smoothstep(radius - aa, radius + aa, d);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * grid.size.xy - grid.params.xy;
    let spacing = grid.params.z;
    let minor = dots(p, spacing, 1.0) * smoothstep(6.0, 14.0, spacing);
    let major = dots(p, spacing * 5.0, 1.5);
    return vec4<f32>(grid.color.rgb, max(minor * 0.6, major) * grid.color.a);
}
