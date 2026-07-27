@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> flash_amount: f32;

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
    let color = textureSample(screen_texture, samp, in.uv);
    return mix(color, vec4(1.0), flash_amount);
}
