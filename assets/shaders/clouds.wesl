// The intro's clouds (src/intro/clouds.rs), after Sebastian Lague's raymarched clouds:
// the box is drawn from the inside (its back faces), and each pixel marches the view ray
// through it, reading density from a tiling 3D noise, lit by how much cloud lies between
// each step and the light (Beer-Lambert), with a two-lobed Henyey-Greenstein phase.
// The result is premultiplied: colour scattered towards the eye, alpha = 1 - transmittance.

import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::view;

struct CloudParams {
    box_min: vec4<f32>,
    box_max: vec4<f32>,
    // Towards the light (xyz), strength (w).
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
    // Metres per shape repeat, coverage, density, seconds.
    shape: vec4<f32>,
    // Metres per detail repeat, erosion, wind x, wind z.
    detail: vec4<f32>,
    // View steps, light steps, phase forward, phase back.
    march: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: CloudParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var noise: texture_3d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var noise_sampler: sampler;

const PI: f32 = 3.14159265;
// Rays never march further than this through the deck, so grazing views stay cheap.
const MAX_MARCH: f32 = 900.0;
// Metres over which the clouds thin out towards the box's sides.
const EDGE_FADE: f32 = 220.0;

// Where a ray enters and leaves the box (t_near > t_far: it misses).
fn ray_box(origin: vec3<f32>, dir: vec3<f32>) -> vec2<f32> {
    let inv = 1.0 / dir;
    let t0 = (params.box_min.xyz - origin) * inv;
    let t1 = (params.box_max.xyz - origin) * inv;
    let lo = min(t0, t1);
    let hi = max(t0, t1);
    let near = max(max(lo.x, lo.y), lo.z);
    let far = min(min(hi.x, hi.y), hi.z);
    return vec2<f32>(max(near, 0.0), far);
}

fn density(p: vec3<f32>) -> f32 {
    let lo = params.box_min.xyz;
    let hi = params.box_max.xyz;
    let h = clamp((p.y - lo.y) / (hi.y - lo.y), 0.0, 1.0);
    // Flat-ish bottoms, rounded tops.
    let rise = clamp(h / 0.12, 0.0, 1.0) * clamp((1.0 - h) / 0.5, 0.0, 1.0);
    let edge = min(min(p.x - lo.x, hi.x - p.x), min(p.z - lo.z, hi.z - p.z));
    let fade = clamp(edge / EDGE_FADE, 0.0, 1.0);

    let wind = vec3<f32>(params.detail.z, 0.0, params.detail.w) * params.shape.w;
    let s = textureSampleLevel(noise, noise_sampler, (p + wind) / params.shape.x, 0.0);
    let coverage = params.shape.y;
    let shape = s.r * rise * fade - coverage;
    if (shape <= 0.0) {
        return 0.0;
    }
    let d = textureSampleLevel(noise, noise_sampler, (p + wind * 1.7) / params.detail.x, 0.0);
    // Three Worley octaves, each twice as fine and half as strong.
    let detail = d.g * 0.57 + d.b * 0.29 + d.a * 0.14;
    // Erode the thin edges most, the cores hardly at all.
    let eroded = shape - (1.0 - detail) * params.detail.y * (1.0 - clamp(shape * 4.0, 0.0, 1.0));
    return max(eroded, 0.0) / max(1.0 - coverage, 0.05) * params.shape.z;
}

// How much light reaches `p` through the cloud towards the light.
fn light_reaching(p: vec3<f32>) -> f32 {
    let dir = params.light_dir.xyz;
    let span = ray_box(p, dir);
    let steps = max(params.march.y, 1.0);
    let step = min(span.y, 250.0) / steps;
    var depth = 0.0;
    for (var i = 0; i < i32(steps); i++) {
        depth += density(p + dir * step * (f32(i) + 0.5)) * step;
    }
    // Beer, with a floor for the light that scatters in from around (thick cloud is
    // never pitch black inside), and a little "powder" darkening of the sunny edges.
    let beer = max(exp(-depth), exp(-depth * 0.25) * 0.7);
    let powder = 1.0 - exp(-depth * 2.0);
    return beer * mix(1.0, powder, 0.3);
}

fn henyey_greenstein(cos_angle: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(1.0 + g2 - 2.0 * g * cos_angle, 1.5));
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let eye = view.world_position;
    let to_frag = in.world_position.xyz - eye;
    let dir = normalize(to_frag);
    let span = ray_box(eye, dir);
    let near = span.x;
    let far = min(min(span.y, length(to_frag)), near + MAX_MARCH);
    if (far <= near) {
        discard;
    }

    let steps = max(params.march.x, 1.0);
    let step = (far - near) / steps;
    // Start each pixel a random part of a step in, so the steps blur into grain rather
    // than banding (the retro dither takes care of the grain).
    var t = near + step * hash(in.position.xy);

    let cos_angle = dot(dir, params.light_dir.xyz);
    let phase = 0.8 + 2.0 * mix(
        henyey_greenstein(cos_angle, params.march.z),
        henyey_greenstein(cos_angle, params.march.w),
        0.5,
    );
    let sun = params.light_color.rgb * params.light_dir.w;

    var transmittance = 1.0;
    var light = vec3<f32>(0.0);
    for (var i = 0; i < i32(steps); i++) {
        if (t > far || transmittance < 0.02) {
            break;
        }
        let p = eye + dir * t;
        let d = density(p);
        if (d > 0.0) {
            let h = clamp((p.y - params.box_min.y) / (params.box_max.y - params.box_min.y), 0.0, 1.0);
            let lit = sun * light_reaching(p) * phase + params.ambient.rgb * (0.4 + 0.6 * h);
            light += lit * d * step * transmittance;
            transmittance *= exp(-d * step);
        }
        t += step;
    }
    let alpha = 1.0 - transmittance;
    if (alpha < 0.002) {
        discard;
    }
    return vec4<f32>(light, alpha);
}
