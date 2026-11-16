//! Fluffy clouds for the intro, after Sebastian Lague's raymarched clouds
//! (github.com/SebLague/Clouds): a box in the sky drawn by a shader that marches the view
//! ray through it, reading density from a tiling 3D noise texture, darkening by how much
//! cloud lies between each step and the light (Beer-Lambert), with a forward-scattering
//! phase function and the "powder" darkening of thick cloud edges.
//!
//! The noise is made here on the CPU ([`cloud_noise`], pure and tested): red is the
//! shape (Worley cells over value noise: billows), green, blue and alpha are Worley at
//! 4, 8 and 16 cells, summed by the shader to erode the edges. It takes a moment, so it is made off the main thread
//! when the game starts, and the clouds show once it is done.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, Face, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};

use crate::general::systems::lights_systems::LightingPreset;

/// Texels along each side of the noise cube.
const NOISE_SIZE: usize = 64;

/// The shape and light of the clouds, as the shader reads them.
#[derive(Debug, Clone, Copy, ShaderType)]
pub struct CloudParams {
    pub box_min: Vec4,
    pub box_max: Vec4,
    /// Towards the light (xyz), and its strength (w).
    pub light_dir: Vec4,
    pub light_color: Vec4,
    /// The sky's light on the shadowed sides (rgb).
    pub ambient: Vec4,
    /// Metres per repeat of the shape noise (x), coverage threshold (y), density (z),
    /// seconds (w, drives the wind).
    pub shape: Vec4,
    /// Metres per repeat of the detail noise (x), how much it erodes (y), wind (zw).
    pub detail: Vec4,
    /// Steps along the view (x), towards the light (y), phase forward/back (zw).
    pub march: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CloudMaterial {
    #[uniform(0)]
    pub params: CloudParams,
    #[texture(1, dimension = "3d")]
    #[sampler(2)]
    pub noise: Handle<Image>,
}

impl Material for CloudMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/clouds.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// Drawn from the inside: the back faces, so the camera can fly through the box.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// The finished noise texture, once made.
#[derive(Resource, Default)]
pub struct CloudNoise {
    pub image: Option<Handle<Image>>,
    task: Option<Task<Vec<u8>>>,
}

/// The cloud box of a running intro.
#[derive(Component)]
pub struct CloudLayer;

pub fn start_making_noise(mut noise: ResMut<CloudNoise>) {
    if noise.image.is_none() && noise.task.is_none() {
        noise.task = Some(AsyncComputeTaskPool::get().spawn(async { cloud_noise(NOISE_SIZE, 7) }));
    }
}

pub fn finish_noise(mut noise: ResMut<CloudNoise>, mut images: ResMut<Assets<Image>>) {
    let Some(task) = noise.task.as_mut() else {
        return;
    };
    let Some(data) = block_on(poll_once(task)) else {
        return;
    };
    noise.task = None;
    let side = NOISE_SIZE as u32;
    let mut image = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: side,
        },
        TextureDimension::D3,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    noise.image = Some(images.add(image));
    info!("intro: cloud noise ready");
}

/// The light the clouds are lit by under a lighting preset: colour, strength, ambient.
pub fn cloud_light(preset: LightingPreset) -> (Vec3, f32, Vec3) {
    match preset {
        // Moonlit: cold silver tops, blue-black undersides.
        LightingPreset::Night => (Vec3::new(0.62, 0.7, 0.9), 1.1, Vec3::new(0.05, 0.07, 0.13)),
        LightingPreset::Dark => (Vec3::new(0.45, 0.5, 0.65), 0.7, Vec3::new(0.02, 0.025, 0.05)),
        LightingPreset::Day => (Vec3::new(1.0, 0.96, 0.9), 2.2, Vec3::new(0.45, 0.55, 0.7)),
    }
}

/// The cloud params for a deck spanning `min`..`max`, lit from `towards_light`.
pub fn cloud_params(min: Vec3, max: Vec3, towards_light: Vec3, preset: LightingPreset) -> CloudParams {
    let (color, strength, ambient) = cloud_light(preset);
    CloudParams {
        box_min: min.extend(0.0),
        box_max: max.extend(0.0),
        light_dir: towards_light.normalize_or(Vec3::Y).extend(strength),
        light_color: color.extend(1.0),
        ambient: ambient.extend(1.0),
        shape: Vec4::new(320.0, 0.58, 0.05, 0.0),
        detail: Vec4::new(55.0, 0.35, 3.0, 1.2),
        march: Vec4::new(56.0, 6.0, 0.6, -0.25),
    }
}

/// Spawn the cloud box from `min` to `max`.
pub fn spawn_clouds(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CloudMaterial>,
    noise: Handle<Image>,
    params: CloudParams,
) -> Entity {
    let (min, max) = (params.box_min.truncate(), params.box_max.truncate());
    let size = max - min;
    commands
        .spawn((
            Name::new("Intro clouds"),
            CloudLayer,
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(materials.add(CloudMaterial { params, noise })),
            Transform::from_translation((min + max) * 0.5),
            bevy::light::NotShadowCaster,
            bevy::light::NotShadowReceiver,
        ))
        .id()
}

/// Wind: advance the clouds' clock.
pub fn drift_clouds(
    time: Res<Time>,
    layers: Query<&MeshMaterial3d<CloudMaterial>, With<CloudLayer>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
) {
    for handle in &layers {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.params.shape.w = time.elapsed_secs();
        }
    }
}

// ---------------------------------------------------------------------------------------
// The noise (pure).

fn hash(x: u32, y: u32, z: u32, seed: u32) -> u32 {
    let mut h = x
        .wrapping_mul(0x8da6_b343)
        ^ y.wrapping_mul(0xd816_3841)
        ^ z.wrapping_mul(0xcb1a_b31f)
        ^ seed.wrapping_mul(0x1656_67b1);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

fn unit(h: u32) -> f32 {
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

/// Distance to the nearest feature point, one per cell of a `cells`-cube that wraps, at
/// `p` in 0..1; 0 at a point, about 1 between them, inverted to 1 - d (cells billow).
fn worley(p: Vec3, cells: u32, seed: u32) -> f32 {
    let c = cells as i32;
    let q = p * cells as f32;
    let base = q.floor();
    let mut nearest = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let cell = base + Vec3::new(dx as f32, dy as f32, dz as f32);
                let wrap = |v: f32| (v as i32).rem_euclid(c) as u32;
                let (x, y, z) = (wrap(cell.x), wrap(cell.y), wrap(cell.z));
                let feature = cell
                    + Vec3::new(
                        unit(hash(x, y, z, seed)),
                        unit(hash(x, y, z, seed ^ 0x9e37)),
                        unit(hash(x, y, z, seed ^ 0x7f4a)),
                    );
                nearest = nearest.min(feature.distance_squared(q));
            }
        }
    }
    1.0 - nearest.sqrt().min(1.0)
}

/// Smooth value noise on a `cells`-cube lattice that wraps.
fn value(p: Vec3, cells: u32, seed: u32) -> f32 {
    let c = cells as i32;
    let q = p * cells as f32;
    let base = q.floor();
    let f = q - base;
    let s = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let corner = |dx: i32, dy: i32, dz: i32| {
        let wrap = |v: f32, d: i32| (v as i32 + d).rem_euclid(c) as u32;
        unit(hash(wrap(base.x, dx), wrap(base.y, dy), wrap(base.z, dz), seed))
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), s.x);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), s.x);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), s.x);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), s.x);
    lerp(lerp(x00, x10, s.y), lerp(x01, x11, s.y), s.z)
}

/// Three octaves of Worley, each twice as fine and half as strong.
fn worley_fbm(p: Vec3, cells: u32, seed: u32) -> f32 {
    worley(p, cells, seed) * 0.625
        + worley(p, cells * 2, seed ^ 1) * 0.25
        + worley(p, cells * 4, seed ^ 2) * 0.125
}

/// The cloud noise: a `size`-cube of RGBA8 texels (x fastest, then y, then z) that tiles
/// in every direction. R = shape, G/B/A = finer and finer erosion.
pub fn cloud_noise(size: usize, seed: u32) -> Vec<u8> {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut shapes = Vec::with_capacity(size * size * size);
    let mut erosion = Vec::with_capacity(size * size * size);
    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let p = (Vec3::new(x as f32, y as f32, z as f32) + 0.5) / size as f32;
                let billow = worley_fbm(p, 3, seed);
                let soft = value(p, 4, seed ^ 0x55) * 0.6 + value(p, 8, seed ^ 0x66) * 0.4;
                // Perlin-Worley in spirit: billowing cells, their sizes varied by the soft
                // noise.
                shapes.push(billow * 0.7 + soft * 0.3);
                // One Worley octave each; the shader sums them.
                erosion.push([
                    byte(worley(p, 4, seed ^ 0x11)),
                    byte(worley(p, 8, seed ^ 0x22)),
                    byte(worley(p, 16, seed ^ 0x33)),
                ]);
            }
        }
    }
    // The mix bunches up round the middle; stretched to the full range, the shader's
    // coverage reads as "about this much of the sky".
    let lo = shapes.iter().copied().fold(f32::MAX, f32::min);
    let hi = shapes.iter().copied().fold(f32::MIN, f32::max);
    let span = (hi - lo).max(1e-4);
    shapes
        .iter()
        .zip(&erosion)
        .flat_map(|(shape, [g, b, a])| [byte((shape - lo) / span), *g, *b, *a])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_noise_has_a_texel_per_cell_and_uses_its_range() {
        let n = cloud_noise(8, 1);
        assert_eq!(n.len(), 8 * 8 * 8 * 4);
        let reds: Vec<u8> = n.iter().step_by(4).copied().collect();
        let (lo, hi) = (reds.iter().min().unwrap(), reds.iter().max().unwrap());
        assert!(*lo < 5 && *hi > 250, "shape noise spans {lo}..{hi}");
    }

    #[test]
    fn worley_wraps_round_the_cube() {
        for cells in [3, 4, 8] {
            let a = worley(Vec3::new(0.0, 0.3, 0.6), cells, 5);
            let b = worley(Vec3::new(1.0, 0.3, 0.6), cells, 5);
            assert!((a - b).abs() < 1e-4, "{cells} cells: {a} vs {b}");
        }
    }

    #[test]
    fn value_noise_wraps_and_stays_in_range() {
        let a = value(Vec3::new(0.2, 0.0, 0.9), 4, 3);
        let b = value(Vec3::new(0.2, 1.0, 0.9), 4, 3);
        assert!((a - b).abs() < 1e-4);
        for i in 0..50 {
            let v = value(Vec3::splat(i as f32 * 0.137), 8, 9);
            assert!((0.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn the_same_seed_makes_the_same_noise() {
        assert_eq!(cloud_noise(4, 2), cloud_noise(4, 2));
        assert_ne!(cloud_noise(4, 2), cloud_noise(4, 3));
    }
}
