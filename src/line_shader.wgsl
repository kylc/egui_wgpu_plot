struct VertexOut {
    @location(0) color: vec4<f32>,
    @location(1) norm: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};

struct Uniforms {
    x_range: vec2<f32>,
    y_range: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

const LINE_WIDTH: f32 = 0.002;
const FEATHER: f32 = 0.50;

@vertex
fn vs_main(@location(0) position: vec2<f32>,
           @location(1) norm: vec2<f32>,
           @location(2) color: vec4<f32>) -> VertexOut {
    var out: VertexOut;

    let width = (uniforms.x_range[1] - uniforms.x_range[0]);
    let height = (uniforms.y_range[1] - uniforms.y_range[0]);

    // Convert from data space (x0..x1, y0..y1) to view space (-1..1, -1..1).
    let x = mix(-1.0, 1.0, (position.x - uniforms.x_range[0]) / width);
    let y = mix(-1.0, 1.0, (position.y - uniforms.y_range[0]) / height);

    // Move the point along the normal by LINE_WIDTH. If the normals are
    // provided such that they are sequentially flipped, this forms a triangle
    // strip the width of the line.
    let delta = vec4(LINE_WIDTH * norm, 0.0, 0.0);

    out.color = color;
    out.norm = norm;
    out.position = vec4<f32>(x, y, 0.0, 1.0) + delta;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // At the edge of the line (final FEATHER % width) feather out the alpha
    // channel to zero.
    let alpha = smoothstep(0.0, 1.0, (1.0 - length(in.norm)) / FEATHER);

    return vec4(in.color.xyz, alpha * in.color.w);
}
