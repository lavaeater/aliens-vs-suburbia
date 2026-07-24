//! Fire fields: a patch of burning ground that damages creatures standing in it over
//! time, throws up flickering flames, and leaves a scorch mark when it burns out.
//!
//! Driven by the [`SpawnFire`] message so any source can start one — the Molotov
//! ability now, thrown molotovs/other incendiaries later. Damage is emitted as
//! [`DamageDealt`] with [`DamageKind::Fire`], which the blood system skips (burns
//! don't spray) while still feeding death/score.

use avian3d::prelude::Position;
use bevy::prelude::*;

use crate::alien::components::general::Alien;
use crate::general::components::Health;
use crate::gore::components::{DamageDealt, DamageKind, Ephemeral, GoreBudget};
use crate::player::components::Player;

/// Start a fire field. Position is on the ground; radius/duration/dps shape it.
#[derive(Message, Clone, Copy, Debug)]
pub struct SpawnFire {
    pub position: Vec3,
    pub radius: f32,
    pub duration: f32,
    pub dps: f32,
}

/// A live patch of fire.
#[derive(Component)]
pub struct FireField {
    life: Timer,
    /// Applies damage on each fire of this timer (repeating).
    damage_tick: Timer,
    /// Spawns a flame puff on each fire (repeating).
    flame_tick: Timer,
    dps: f32,
    radius: f32,
}

/// Shared fire art.
#[derive(Resource)]
pub struct FireAssets {
    flame_mesh: Handle<Mesh>,
    scorch_mesh: Handle<Mesh>,
    scorch_material: Handle<StandardMaterial>,
}

pub fn setup_fire_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let flame_mesh = meshes.add(Mesh::from(Sphere::new(0.5)));
    let scorch_mesh = meshes.add(Mesh::from(Plane3d::new(Vec3::Y, Vec2::splat(0.5))));
    let scorch_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.05, 0.04, 0.03, 0.9),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        depth_bias: 1.0,
        ..default()
    });
    commands.insert_resource(FireAssets {
        flame_mesh,
        scorch_mesh,
        scorch_material,
    });
}

/// Spawn a `FireField` entity for each `SpawnFire`, with a glowing base blob.
pub fn spawn_fire_fields(
    mut msgs: MessageReader<SpawnFire>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for fire in msgs.read() {
        let base_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.35, 0.05, 0.5),
            emissive: LinearRgba::new(5.0, 1.6, 0.2, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            FireField {
                life: Timer::from_seconds(fire.duration, TimerMode::Once),
                damage_tick: Timer::from_seconds(0.25, TimerMode::Repeating),
                flame_tick: Timer::from_seconds(0.08, TimerMode::Repeating),
                dps: fire.dps,
                radius: fire.radius,
            },
            Mesh3d(meshes.add(Mesh::from(Sphere::new(0.5)))),
            MeshMaterial3d(base_mat),
            Transform::from_translation(fire.position + Vec3::Y * 0.15)
                .with_scale(Vec3::splat(fire.radius * 1.4)),
        ));
    }
}

/// xorshift32 for flame jitter.
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

/// Burn things in the fire, flicker the flames, and scorch the ground on burnout.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tick_fire_fields(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut budget: ResMut<GoreBudget>,
    fire_assets: Option<Res<FireAssets>>,
    mut fields: Query<(Entity, &mut FireField, &Transform)>,
    mut targets: Query<(Entity, &Position, &mut Health, Has<Alien>, Has<Player>)>,
    mut damage_mw: MessageWriter<DamageDealt>,
    mut rng_seed: Local<u32>,
) {
    let dt = time.delta();
    for (entity, mut field, transform) in fields.iter_mut() {
        field.life.tick(dt);
        field.damage_tick.tick(dt);
        field.flame_tick.tick(dt);

        let center = transform.translation;

        // ── Burn creatures inside the radius on each damage tick. ──────────────
        if field.damage_tick.just_finished() {
            let amount = (field.dps * field.damage_tick.duration().as_secs_f32()) as i32;
            if amount > 0 {
                let r2 = field.radius * field.radius;
                for (target, pos, mut health, is_alien, is_player) in targets.iter_mut() {
                    if !(is_alien || is_player) {
                        continue;
                    }
                    if pos.0.distance_squared(center) <= r2 {
                        health.health -= amount;
                        damage_mw.write(DamageDealt {
                            target,
                            position: pos.0 + Vec3::Y * 0.4,
                            normal: Vec3::Y,
                            amount,
                            kind: DamageKind::Fire,
                            lethal: health.health <= 0,
                        });
                    }
                }
            }
        }

        // ── Flame puffs: small rising, fading emissive blobs. ──────────────────
        if field.flame_tick.just_finished() {
            *rng_seed = rng_seed.wrapping_add(0x9E3779B9);
            let mut rng = Rng(*rng_seed ^ entity.to_bits() as u32);
            let off = Vec3::new(rng.signed(), 0.0, rng.signed()) * field.radius * 0.7;
            let size = 0.25 + rng.unit() * 0.35;
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.5, 0.1, 0.9),
                emissive: LinearRgba::new(6.0, 2.2, 0.3, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            });
            let fa = fire_assets.as_ref();
            let mesh = fa
                .map(|a| a.flame_mesh.clone())
                .unwrap_or_else(|| meshes.add(Mesh::from(Sphere::new(0.5))));
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_translation(center + off + Vec3::Y * 0.2)
                    .with_scale(Vec3::splat(size)),
                Ephemeral::new(0.35).with_grow(1.8).base_scale(Vec3::splat(size)),
            ));
        }

        // ── Burn out: scorch the ground, then despawn. ─────────────────────────
        if field.life.is_finished() {
            if let Some(fa) = fire_assets.as_ref() {
                let scorch = commands
                    .spawn((
                        Mesh3d(fa.scorch_mesh.clone()),
                        MeshMaterial3d(fa.scorch_material.clone()),
                        Transform::from_xyz(center.x, 0.02, center.z)
                            .with_scale(Vec3::splat(field.radius * 2.2)),
                    ))
                    .id();
                if let Some(evicted) = budget.push_decal(scorch) {
                    commands.entity(evicted).despawn();
                }
            }
            commands.entity(entity).despawn();
        }
    }
}
