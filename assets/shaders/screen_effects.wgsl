#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct ScreenEffectSettings {
    chromatic_intensity: f32,
    flash_amount: f32,
    circle_wipe_progress: f32,
#ifdef SIXTEEN_BYTE_ALIGNMENT
    _webgl2_padding: f32,
#endif
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: ScreenEffectSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(screen_texture, texture_sampler, in.uv);

    // Chromatic aberration
    let chroma = settings.chromatic_intensity;
    if chroma > 0.0 {
        let offset = chroma * 0.02;
        let r = textureSample(screen_texture, texture_sampler, in.uv + vec2<f32>(offset, 0.0)).r;
        let g = textureSample(screen_texture, texture_sampler, in.uv).g;
        let b = textureSample(screen_texture, texture_sampler, in.uv - vec2<f32>(offset, 0.0)).b;
        color = vec4<f32>(r, g, b, color.a);
    }

    // Flash white overlay
    if settings.flash_amount > 0.0 {
        color = mix(color, vec4<f32>(1.0, 1.0, 1.0, 1.0), settings.flash_amount);
    }

    // Circle wipe transition
    if settings.circle_wipe_progress > 0.0 {
        let center = vec2<f32>(0.5, 0.5);
        let dist = distance(in.uv, center);
        let radius = sqrt(0.5) * settings.circle_wipe_progress;
        if dist > radius {
            color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }

    return color;
}
