@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> progress: f32;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let positions = array(
        vec4(-1.0, -1.0, 0.0, 1.0),
        vec4( 1.0, -1.0, 0.0, 1.0),
        vec4(-1.0,  1.0, 0.0, 1.0),
        vec4( 1.0,  1.0, 0.0, 1.0),
    );
    let uvs = array(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
    );
    var out: VertexOutput;
    out.position = positions[vi];
    out.uv = uvs[vi];
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2(0.5, 0.5);
    let dist = distance(in.uv, center);
    let radius = sqrt(0.5) * progress;
    let alpha = 1.0 - smoothstep(radius - 0.05, radius + 0.05, dist);
    let color = textureSample(screen_texture, samp, in.uv);
    return vec4(color.rgb, alpha);
}
