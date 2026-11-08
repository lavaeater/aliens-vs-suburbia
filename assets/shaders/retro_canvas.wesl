// The retro canvas (src/retro/mod.rs): draws the low-res scene on the window, one texel
// at a time: graded in OKLab (exposure, contrast, saturation, violet shadows, warm
// highlights), inked where it is darker than a neighbour, then matched to a palette with
// an 8x8 ordered (Bayer) dither between the two nearest colours.

import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;

const MAX_COLORS: u32 = 256u;

struct RetroParams {
    image_size: vec2<f32>,
    // The camera's texel index mod 8 (x right, y down), so the pattern stays on the world.
    pattern_offset: vec2<f32>,
    dither: f32,
    exposure: f32,
    palette_len: u32,
    _pad: u32,
    contrast: f32,
    saturation: f32,
    shadow_tint: f32,
    warmth: f32,
    ink: f32,
    ink_threshold: f32,
    _pad2: f32,
    _pad3: f32,
    palette_lab: array<vec4<f32>, 256>,
    palette_linear: array<vec4<f32>, 256>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: RetroParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var scene: texture_2d<f32>;

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

fn oklab_to_linear(c: vec3<f32>) -> vec3<f32> {
    let l_ = c.x + 0.3963377774 * c.y + 0.2158037573 * c.z;
    let m_ = c.x - 0.1055613458 * c.y - 0.0638541728 * c.z;
    let s_ = c.x - 0.0894841775 * c.y - 1.2914855480 * c.z;
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;
    return vec3<f32>(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    );
}

// Where the shadows lean (violet, OKLCh hue ~290) and the highlights (warm, ~75).
const SHADOW_AB: vec2<f32> = vec2<f32>(0.026, -0.072);
const WARM_AB: vec2<f32> = vec2<f32>(0.013, 0.048);

fn lightness(texel: vec2<i32>) -> f32 {
    let size = vec2<i32>(params.image_size);
    let c = textureLoad(scene, clamp(texel, vec2<i32>(0), size - 1), 0).rgb * params.exposure;
    return linear_to_oklab(c).x;
}

// The scene colour at `texel`, graded, in OKLab.
fn graded(texel: vec2<i32>) -> vec3<f32> {
    let colour = textureLoad(scene, texel, 0).rgb * params.exposure;
    var lab = linear_to_oklab(colour);
    lab.x = clamp((lab.x - 0.5) * params.contrast + 0.5, 0.0, 1.0);
    var ab = lab.yz * params.saturation;
    let dark = (1.0 - lab.x) * (1.0 - lab.x);
    ab += SHADOW_AB * params.shadow_tint * dark;
    lab.x += 0.05 * params.shadow_tint * dark * dark;
    ab += WARM_AB * params.warmth * lab.x * lab.x;
    lab = vec3<f32>(lab.x, ab);

    // Ink: a line on the dark side of every edge, so it is one texel thick.
    if params.ink > 0.0 {
        let here = lightness(texel);
        var rise = 0.0;
        rise = max(rise, lightness(texel + vec2<i32>(1, 0)) - here);
        rise = max(rise, lightness(texel - vec2<i32>(1, 0)) - here);
        rise = max(rise, lightness(texel + vec2<i32>(0, 1)) - here);
        rise = max(rise, lightness(texel - vec2<i32>(0, 1)) - here);
        if rise > params.ink_threshold {
            let ink = vec3<f32>(0.16, SHADOW_AB * 0.6);
            lab = mix(lab, ink, params.ink);
        }
    }
    return lab;
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
    let lab = graded(texel);

    let count = min(params.palette_len, MAX_COLORS);
    if count == 0u {
        return vec4<f32>(max(oklab_to_linear(lab), vec3<f32>(0.0)), 1.0);
    }

    // The two palette colours nearest in OKLab.
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
