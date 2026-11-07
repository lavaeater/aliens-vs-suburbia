// The retro canvas (src/retro/mod.rs): draws the low-res scene on the window, one texel
// at a time, matched to a palette in OKLab with an 8x8 ordered (Bayer) dither between the
// two nearest colours.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

const MAX_COLORS: u32 = 256u;

struct RetroParams {
    image_size: vec2<f32>,
    // The camera's texel index mod 8 (x right, y down), so the pattern stays on the world.
    pattern_offset: vec2<f32>,
    dither: f32,
    exposure: f32,
    palette_len: u32,
    _pad: u32,
    palette_lab: array<vec4<f32>, 256>,
    palette_linear: array<vec4<f32>, 256>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: RetroParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var scene: texture_2d<f32>;

fn linear_to_oklab(c: vec3<f32>) -> vec3<f32> {
    let l = 0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b;
    let m = 0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b;
    let s = 0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b;
    let l_ = pow(max(l, 0.0), 1.0 / 3.0);
    let m_ = pow(max(m, 0.0), 1.0 / 3.0);
    let s_ = pow(max(s, 0.0), 1.0 / 3.0);
    return vec3<f32>(
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    );
}

// 8x8 Bayer threshold in 0..1: bit-reverse of the interleaved (x ^ y, y) bits.
fn bayer8(p: vec2<u32>) -> f32 {
    let x = p.x & 7u;
    let y = p.y & 7u;
    let a = x ^ y;
    let v = ((a & 1u) << 5u) | ((y & 1u) << 4u)
        | ((a & 2u) << 2u) | ((y & 2u) << 1u)
        | ((a & 4u) >> 1u) | ((y & 4u) >> 2u);
    return (f32(v) + 0.5) / 64.0;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<i32>(params.image_size);
    let texel = clamp(vec2<i32>(floor(in.uv * params.image_size)), vec2<i32>(0), size - 1);
    let colour = textureLoad(scene, texel, 0).rgb * params.exposure;

    let count = min(params.palette_len, MAX_COLORS);
    if count == 0u {
        return vec4<f32>(colour, 1.0);
    }

    // The two palette colours nearest in OKLab.
    let lab = linear_to_oklab(colour);
    var best = 0u;
    var second = 0u;
    var best_d = 1e9;
    var second_d = 1e9;
    for (var i = 0u; i < count; i++) {
        let d = distance(lab, params.palette_lab[i].xyz);
        if d < best_d {
            second = best;
            second_d = best_d;
            best = i;
            best_d = d;
        } else if d < second_d {
            second = i;
            second_d = d;
        }
    }

    // How far along best -> second the colour sits; dither picks between them by that.
    var pick = best;
    if count > 1u && params.dither > 0.0 {
        let a = params.palette_lab[best].xyz;
        let b = params.palette_lab[second].xyz;
        let ab = b - a;
        let t = clamp(dot(lab - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
        let pattern = vec2<u32>(vec2<i32>(texel) + vec2<i32>(params.pattern_offset));
        let threshold = 0.5 + (bayer8(pattern) - 0.5) * params.dither;
        if t > threshold {
            pick = second;
        }
    }
    return vec4<f32>(params.palette_linear[pick].rgb, 1.0);
}
