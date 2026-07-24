//! Gibs: on death, burst the corpse into a spray of physics chunks that tumble away
//! from the killing blow, then fade out. Subscribes to [`EntityDied`]; budget-capped
//! via [`GoreBudget`].
//!
//! Chunks are procedural (small dark-red cuboids) so there's no art dependency — a
//! generic "man-beast" gore burst. Per-enemy gib sets can come later via `EnemyProps`.

use avian3d::prelude::{
    AngularVelocity, Collider, CollisionLayers, LinearVelocity, RigidBody,
};
use bevy::prelude::*;

use crate::general::components::CollisionLayer;
use crate::gore::components::{Ephemeral, EntityDied, GoreBudget};

/// Shared gib mesh + material, built once at startup.
#[derive(Resource)]
pub struct GibAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

/// How many chunks a corpse bursts into.
const GIB_COUNT: u32 = 6;
/// How long gibs linger before fading away.
const GIB_LIFETIME: f32 = 6.0;

pub fn setup_gib_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.03, 0.03),
        perceptual_roughness: 0.75,
        ..default()
    });
    commands.insert_resource(GibAssets { mesh, material });
}

/// Deterministic per-death PRNG (xorshift32) so a burst looks varied but is cheap.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// In [-1, 1].
    fn signed(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
    /// In [0, 1].
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

pub fn spawn_gibs_on_death(
    mut deaths: MessageReader<EntityDied>,
    mut commands: Commands,
    mut budget: ResMut<GoreBudget>,
    gibs: Option<Res<GibAssets>>,
) {
    let Some(gibs) = gibs else { return };

    for death in deaths.read() {
        let mut rng = Rng(death.entity.to_bits() as u32 ^ 0x6165);
        // Bias the burst away from the killing blow, upward and outward.
        let base_dir = (death.normal.normalize_or(Vec3::Y) + Vec3::Y * 0.6).normalize_or(Vec3::Y);

        for _ in 0..GIB_COUNT {
            let jitter = Vec3::new(rng.signed(), rng.signed() * 0.5 + 0.5, rng.signed());
            let dir = (base_dir + jitter * 0.8).normalize_or(Vec3::Y);
            let speed = 3.0 + rng.unit() * 4.0;

            // Small, slightly irregular chunk.
            let size = 0.07 + rng.unit() * 0.09;
            let scale = Vec3::new(
                size * (0.7 + rng.unit() * 0.6),
                size * (0.7 + rng.unit() * 0.6),
                size * (0.7 + rng.unit() * 0.6),
            );
            let spawn = death.position + Vec3::Y * 0.4 + jitter * 0.15;

            let gib = commands
                .spawn((
                    Mesh3d(gibs.mesh.clone()),
                    MeshMaterial3d(gibs.material.clone()),
                    Transform::from_translation(spawn).with_scale(scale),
                    RigidBody::Dynamic,
                    // Unit collider; the transform scale sizes both mesh and collider.
                    Collider::cuboid(1.0, 1.0, 1.0),
                    // Gibs bounce off the world but don't shove aliens/players around.
                    CollisionLayers::new(
                        [CollisionLayer::Ball],
                        [CollisionLayer::Floor, CollisionLayer::ImpassableAll],
                    ),
                    LinearVelocity(dir * speed),
                    AngularVelocity(Vec3::new(rng.signed(), rng.signed(), rng.signed()) * 12.0),
                    // No fade (opaque chunks) — just linger, then vanish.
                    Ephemeral::new(GIB_LIFETIME).no_fade().base_scale(scale),
                ))
                .id();

            if let Some(evicted) = budget.push_gib(gib) {
                commands.entity(evicted).despawn();
            }
        }
    }
}
