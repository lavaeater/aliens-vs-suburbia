#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0) var sprite_texture: texture_2d<f32>;
@group(2) @binding(1) var sprite_sampler: sampler;
// uv_rect: (offset_u, offset_v, width_u, height_v) all in [0..1] UV space
@group(2) @binding(2) var<uniform> uv_rect: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let u = uv_rect.x + in.uv.x * uv_rect.z;
    let v = uv_rect.y + in.uv.y * uv_rect.w;
    let color = textureSample(sprite_texture, sprite_sampler, vec2<f32>(u, v));
    if color.a < 0.05 {
        discard;
    }
    return color;
}
