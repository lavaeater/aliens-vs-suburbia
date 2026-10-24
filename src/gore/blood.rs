//! Blood: a particle *spray* at the moment of impact (`crate::fx`, the effects are
//! `assets/fx/blood_*.ron`), plus a persistent *decal* stain dropped on the ground that
//! dirties up the map. Both subscribe to [`DamageDealt`]; the decal is budget-capped
//! via [`GoreBudget`]. Structures don't bleed: a shot that hits one throws sparks
//! (`shoot_weapons`).
//!
//! The splat texture is generated procedurally at startup, so the decal has no art
//! dependency — irregular crimson blob with a soft edge and a scatter of droplets.
//!
//! The stains are alpha-*masked*, not blended. Blended, they went into the transparent
//! pass, which sorts by distance only, so a stain sorted after a translucent wall or a
//! faded mesh painted straight over it. Masked, they draw in the opaque pass with a
//! real depth test.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::fx::{FxKind, SpawnFx};
use crate::general::damage::Faction;
use crate::general::systems::map_systems::TileDefinitions;
use crate::gore::components::{DECAL_LIFT, DamageDealt, DamageKind, GoreBudget};

/// Shared, pre-built blood art.
#[derive(Resource)]
pub struct BloodAssets {
    /// Flat quad facing +Y, one world unit across, for ground decals.
    decal_mesh: Handle<Mesh>,
    /// Shared decal material (alpha-masked) — every stain reuses it, so they batch.
    decal_material: Handle<StandardMaterial>,
}

const TEX_SIZE: u32 = 64;

/// Deterministic little PRNG so the texture looks organic without pulling in a dep.
struct Rng(u32);
impl Rng {
    const fn next(&mut self) -> u32 {
        // xorshift32
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn f32(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Build the crimson splat: an irregular central blob (sum of a few sine lobes on
/// the radius) plus a handful of satellite droplets, with a soft alpha edge.
fn make_blood_texture() -> Image {
    let n = TEX_SIZE as usize;
    let mut data = vec![0u8; n * n * 4];
    let mut rng = Rng(0xB100D);

    // A few droplets: (cx, cy, radius) in [0,1] texel space.
    let mut droplets = [(0.0f32, 0.0f32, 0.0f32); 6];
    for d in &mut droplets {
        let ang = rng.f32() * std::f32::consts::TAU;
        let dist = 0.28 + rng.f32() * 0.20;
        d.0 = 0.5 + ang.cos() * dist;
        d.1 = 0.5 + ang.sin() * dist;
        d.2 = 0.03 + rng.f32() * 0.06;
    }
    // Lobe phases for the irregular main blob.
    let lobe_phase = rng.f32() * std::f32::consts::TAU;

    for y in 0..n {
        for x in 0..n {
            let fx = (x as f32 + 0.5) / n as f32 - 0.5;
            let fy = (y as f32 + 0.5) / n as f32 - 0.5;
            let r = (fx * fx + fy * fy).sqrt();
            let ang = fy.atan2(fx);

            // Irregular boundary: base radius wobbled by a couple of harmonics.
            let edge = 0.34
                + 0.05 * (3.0 * ang + lobe_phase).sin()
                + 0.03 * (5.0 * ang - lobe_phase).sin();
            let mut a = ((edge - r) / 0.06).clamp(0.0, 1.0);

            // Add droplets.
            for &(cx, cy, dr) in &droplets {
                let ddx = fx + 0.5 - cx;
                let ddy = fy + 0.5 - cy;
                let dd = (ddx * ddx + ddy * ddy).sqrt();
                a = a.max(((dr - dd) / 0.03).clamp(0.0, 1.0));
            }

            // Darker toward the middle (pooled), brighter at the thin edges.
            let dark = 0.35 + 0.25 * (r / 0.4).clamp(0.0, 1.0);
            let idx = (y * n + x) * 4;
            if let Some([r_px, g_px, b_px, a_px]) = data.get_mut(idx..idx + 4) {
                *r_px = (0.45 * dark * 255.0) as u8;
                *g_px = (0.02 * 255.0) as u8;
                *b_px = (0.02 * 255.0) as u8;
                *a_px = (a * 255.0) as u8;
            }
        }
    }

    Image::new(
        Extent3d {
            width: TEX_SIZE,
            height: TEX_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Build the shared blood assets once, when the game starts.
pub fn setup_blood_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let texture = images.add(make_blood_texture());
    let decal_mesh = meshes.add(Mesh::from(Plane3d::new(Vec3::Y, Vec2::splat(0.5))));
    let decal_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        base_color_texture: Some(texture.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        unlit: true,
        // Push decals in front of the floor to avoid z-fighting.
        depth_bias: 1.0,
        ..default()
    });

    commands.insert_resource(BloodAssets {
        decal_mesh,
        decal_material,
    });
}

/// Spawn blood on every non-fire hit: a particle spray at the impact and a lasting
/// stain on the ground below it.
pub fn spawn_blood_on_damage(
    mut damage: MessageReader<DamageDealt>,
    mut commands: Commands,
    mut budget: ResMut<GoreBudget>,
    mut fx: MessageWriter<SpawnFx>,
    factions: Query<&Faction>,
    blood: Option<Res<BloodAssets>>,
    tile_defs: Res<TileDefinitions>,
) {
    let Some(blood) = blood else { return };

    for hit in damage.read() {
        // Fire scorches rather than bleeds — leave that to the fire feature.
        if hit.kind == DamageKind::Fire {
            continue;
        }
        if factions.get(hit.target) == Ok(&Faction::Structure) {
            continue;
        }
        // Bigger hits bleed more; a killing blow really lets go.
        let mut mag = ((hit.amount as f32) / 10.0).clamp(0.5, 3.0);
        if hit.lethal {
            mag *= 1.8;
        }

        // ── Impact spray: the effect emits along its +Y, so turn that to the spray
        //    direction, tipped upwards so the droplets arc before they land. ───────
        let spray = hit.normal.normalize_or(Vec3::Y) + Vec3::Y * 0.6;
        let kind = if hit.lethal {
            FxKind::BloodKill
        } else {
            FxKind::BloodHit
        };
        fx.write(SpawnFx::at(kind, hit.position).along(spray));

        // ── Ground stain: persistent, budget-capped. Flat quad just above the floor,
        //    directly under the hit (maps are flat, so the floor plane works without a
        //    raycast -- but that plane is `floor_level`, not 0). ───────────────────────
        let yaw = (hit.position.x * 12.9898 + hit.position.z * 78.233).sin() * 43_758.547;
        let yaw = (yaw - yaw.floor()) * std::f32::consts::TAU;
        // Tiles are one unit wide: a normal hit leaves a stain under half a tile, a
        // killing blow about a tile.
        let jitter = yaw / std::f32::consts::TAU;
        let size = (0.25 + mag * 0.15) * (0.8 + 0.4 * jitter);
        // Masked stains write depth, so two at the same height z-fight where they
        // overlap; stagger them by a hair.
        let lift = DECAL_LIFT + jitter * 0.004;
        let decal = commands
            .spawn((
                Mesh3d(blood.decal_mesh.clone()),
                MeshMaterial3d(blood.decal_material.clone()),
                NotShadowCaster,
                Transform::from_xyz(hit.position.x, tile_defs.floor_level + lift, hit.position.z)
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::splat(size)),
            ))
            .id();
        if let Some(evicted) = budget.push_decal(decal) {
            commands.entity(evicted).despawn();
        }
    }
}
