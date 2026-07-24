//! Destructible terrain. When a map structure (a blocking terrain placement or a
//! tower — anything `IsObstacle` with `Health`) is reduced to zero, this owns its
//! death: it re-opens the tile for pathfinding, bursts it into rubble + dust, and
//! despawns it.
//!
//! This deliberately takes obstacles *out* of `health_monitor_system` (which is now
//! `Without<IsObstacle>`) so structures crumble into rubble instead of flesh gibs, and
//! so their tile re-opens no matter who destroyed them — a bullet, an explosion, or an
//! alien chewing through (aliens already re-open the tiles they personally break).

use avian3d::prelude::{AngularVelocity, Collider, CollisionLayers, LinearVelocity, RigidBody};
use bevy::prelude::*;

use crate::general::components::map_components::CurrentTile;
use crate::general::components::{CollisionLayer, Health, Indestructible};
use crate::general::resources::map_resources::MapGraph;
use crate::gore::components::{Ephemeral, GoreBudget};
use crate::player::components::IsObstacle;

/// Shared rubble/dust art, built once at startup.
#[derive(Resource)]
pub struct DebrisAssets {
    chunk_mesh: Handle<Mesh>,
    chunk_material: Handle<StandardMaterial>,
    dust_mesh: Handle<Mesh>,
}

const CHUNK_COUNT: u32 = 7;
const CHUNK_LIFETIME: f32 = 5.0;

pub fn setup_debris_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let chunk_mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
    let chunk_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.34, 0.32, 0.30),
        perceptual_roughness: 0.95,
        ..default()
    });
    let dust_mesh = meshes.add(Mesh::from(Sphere::new(0.5)));
    commands.insert_resource(DebrisAssets {
        chunk_mesh,
        chunk_material,
        dust_mesh,
    });
}

/// xorshift32, seeded per structure so bursts look varied without a dep.
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
    fn signed(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[allow(clippy::type_complexity)]
pub fn destroy_damaged_terrain(
    mut commands: Commands,
    mut map_graph: ResMut<MapGraph>,
    mut budget: ResMut<GoreBudget>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    debris: Option<Res<DebrisAssets>>,
    structures: Query<
        (Entity, &Health, Option<&CurrentTile>, &Transform),
        (With<IsObstacle>, Without<Indestructible>),
    >,
) {
    let Some(debris) = debris else { return };

    for (entity, health, tile, transform) in structures.iter() {
        if health.health > 0 {
            continue;
        }

        // Re-open the tile so aliens can path through the gap (idempotent if the alien
        // that broke it already did this). `recheck_path_after_tile_opened` reacts.
        if let Some(tile) = tile {
            map_graph.path_finding_grid.add_vertex(tile.tile);
            map_graph.path_reopened = true;
        }

        spawn_debris(&mut commands, &mut materials, &mut budget, &debris, transform);
        commands.entity(entity).despawn();
    }
}

/// A puff of dust and a scatter of rubble chunks where a structure stood.
fn spawn_debris(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    budget: &mut GoreBudget,
    debris: &DebrisAssets,
    transform: &Transform,
) {
    let origin = transform.translation;
    let mut rng = Rng((origin.x * 73.0 + origin.z * 179.0) as i32 as u32 ^ 0xDEB1);

    // Dust: a grey cloud that swells and fades. Own material so the fade is isolated.
    let dust_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.55, 0.52, 0.48, 0.85),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let dust_size = 0.6;
    commands.spawn((
        Mesh3d(debris.dust_mesh.clone()),
        MeshMaterial3d(dust_mat),
        Transform::from_translation(origin + Vec3::Y * 0.3).with_scale(Vec3::splat(dust_size)),
        Ephemeral::new(0.7).with_grow(2.8).base_scale(Vec3::splat(dust_size)),
    ));

    // Rubble: grey physics chunks thrown up and out.
    for _ in 0..CHUNK_COUNT {
        let dir = Vec3::new(rng.signed(), rng.unit() * 0.8 + 0.4, rng.signed()).normalize_or(Vec3::Y);
        let speed = 2.0 + rng.unit() * 3.5;
        let size = 0.08 + rng.unit() * 0.12;
        let scale = Vec3::splat(size) * Vec3::new(0.7 + rng.unit() * 0.6, 0.7 + rng.unit() * 0.6, 0.7 + rng.unit() * 0.6);
        let spawn = origin + Vec3::Y * 0.4 + dir * 0.2;

        let chunk = commands
            .spawn((
                Mesh3d(debris.chunk_mesh.clone()),
                MeshMaterial3d(debris.chunk_material.clone()),
                Transform::from_translation(spawn).with_scale(scale),
                RigidBody::Dynamic,
                Collider::cuboid(1.0, 1.0, 1.0),
                CollisionLayers::new(
                    [CollisionLayer::Ball],
                    [CollisionLayer::Floor, CollisionLayer::ImpassableAll],
                ),
                LinearVelocity(dir * speed),
                AngularVelocity(Vec3::new(rng.signed(), rng.signed(), rng.signed()) * 9.0),
                Ephemeral::new(CHUNK_LIFETIME).no_fade().base_scale(scale),
            ))
            .id();

        if let Some(evicted) = budget.push_gib(chunk) {
            commands.entity(evicted).despawn();
        }
    }
}
