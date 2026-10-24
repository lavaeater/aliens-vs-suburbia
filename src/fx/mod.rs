//! GPU particle effects (`bevy_sprinkles`). Gameplay never spawns a particle system
//! itself: it writes a [`SpawnFx`] (what, where, which way) and [`spawn_fx`] turns it
//! into a `Particles3d` from the shared [`FxAssets`]. The effects are RON files in
//! `assets/fx/`; one-shots despawn themselves when they finish.
//!
//! An effect emits along its own +Y, so [`SpawnFx::along`] turns +Y to the direction
//! given. Looping effects (fire) are spawned by their owner, which keeps the entity and
//! ends it with [`StopFx`]: emission stops at once, the entity goes once the last
//! particle has had time to die.
//!
//! Particles collide with sprinkles' own colliders, not with avian's. There is one,
//! [`FloorParticleCollider`], a box under the whole map whose top is the floor plane
//! (sprinkles allows 32 in the scene, so no per-wall colliders).

use bevy::prelude::*;
use bevy_sprinkles::prelude::{
    EmitterEntity, EmitterRuntime, Particles3d, ParticlesAsset, ParticlesCollider3D,
    ParticlesColliderShape3D, SprinklesPlugin,
};

use crate::general::systems::map_systems::TileDefinitions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxKind {
    BloodHit,
    BloodKill,
    MuzzleSparks,
    ImpactSparks,
    Explosion,
    DustBurst,
}

/// Play a one-shot effect at `at`, its +Y along `dir`.
#[derive(Message, Clone, Copy, Debug)]
pub struct SpawnFx {
    pub kind: FxKind,
    pub at: Vec3,
    pub dir: Vec3,
}

impl SpawnFx {
    pub const fn at(kind: FxKind, at: Vec3) -> Self {
        Self {
            kind,
            at,
            dir: Vec3::Y,
        }
    }

    pub const fn along(mut self, dir: Vec3) -> Self {
        self.dir = dir;
        self
    }
}

/// Every effect, loaded once.
#[derive(Resource)]
pub struct FxAssets {
    blood_hit: Handle<ParticlesAsset>,
    blood_kill: Handle<ParticlesAsset>,
    muzzle_sparks: Handle<ParticlesAsset>,
    impact_sparks: Handle<ParticlesAsset>,
    explosion: Handle<ParticlesAsset>,
    dust_burst: Handle<ParticlesAsset>,
    /// Looping; spawned by `gore::fire` for each fire field.
    pub fire: Handle<ParticlesAsset>,
}

impl FxAssets {
    fn handle(&self, kind: FxKind) -> Handle<ParticlesAsset> {
        match kind {
            FxKind::BloodHit => self.blood_hit.clone(),
            FxKind::BloodKill => self.blood_kill.clone(),
            FxKind::MuzzleSparks => self.muzzle_sparks.clone(),
            FxKind::ImpactSparks => self.impact_sparks.clone(),
            FxKind::Explosion => self.explosion.clone(),
            FxKind::DustBurst => self.dust_burst.clone(),
        }
    }
}

/// Marks the particle floor: a box whose top face is the floor plane, so droplets,
/// sparks and debris come to rest on it instead of falling through the map.
#[derive(Component)]
pub struct FloorParticleCollider;

/// Wide enough to sit under any map; sprinkles colliders are cheap analytic boxes.
const FLOOR_COLLIDER_SIZE: f32 = 1000.0;

/// End a looping effect: stop emitting, despawn after `linger` seconds (at least the
/// longest particle lifetime, so nothing pops).
#[derive(Component)]
pub struct StopFx {
    linger: Timer,
}

impl StopFx {
    pub fn after(linger: f32) -> Self {
        Self {
            linger: Timer::from_seconds(linger, TimerMode::Once),
        }
    }
}

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SprinklesPlugin)
            .add_message::<SpawnFx>()
            .add_systems(Startup, (setup_fx_assets, spawn_floor_particle_collider))
            .add_systems(Update, (spawn_fx, stop_fx));
    }
}

fn setup_fx_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(FxAssets {
        blood_hit: asset_server.load("fx/blood_hit.ron"),
        blood_kill: asset_server.load("fx/blood_kill.ron"),
        muzzle_sparks: asset_server.load("fx/muzzle_sparks.ron"),
        impact_sparks: asset_server.load("fx/impact_sparks.ron"),
        explosion: asset_server.load("fx/explosion.ron"),
        dust_burst: asset_server.load("fx/dust_burst.ron"),
        fire: asset_server.load("fx/fire.ron"),
    });
}

/// Put the particle floor under the map. The floor plane never moves, so this is
/// spawned once and kept across states.
fn spawn_floor_particle_collider(mut commands: Commands, tile_defs: Option<Res<TileDefinitions>>) {
    let Some(tile_defs) = tile_defs else { return };
    commands.spawn((
        Name::new("Particle floor"),
        FloorParticleCollider,
        ParticlesCollider3D {
            enabled: true,
            shape: ParticlesColliderShape3D::Box {
                size: Vec3::new(FLOOR_COLLIDER_SIZE, 1.0, FLOOR_COLLIDER_SIZE),
            },
        },
        Transform::from_xyz(0.0, tile_defs.floor_level - 0.5, 0.0),
    ));
}

/// The rotation that turns an effect's +Y to `dir` (straight up for a zero `dir`).
pub fn facing(dir: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::Y))
}

fn spawn_fx(mut msgs: MessageReader<SpawnFx>, mut commands: Commands, fx: Option<Res<FxAssets>>) {
    let Some(fx) = fx else {
        msgs.clear();
        return;
    };
    for msg in msgs.read() {
        commands.spawn((
            Name::new(format!("Fx {:?}", msg.kind)),
            Particles3d(fx.handle(msg.kind)),
            Transform::from_translation(msg.at).with_rotation(facing(msg.dir)),
        ));
    }
}

fn stop_fx(
    time: Res<Time>,
    mut commands: Commands,
    mut stopping: Query<(Entity, &mut StopFx)>,
    mut emitters: Query<(&EmitterEntity, &mut EmitterRuntime)>,
) {
    for (entity, mut stop) in &mut stopping {
        if stop.linger.elapsed_secs() == 0.0 {
            for (emitter, mut runtime) in &mut emitters {
                if emitter.parent_system == entity {
                    runtime.set_emitting(false);
                }
            }
        }
        stop.linger.tick(time.delta());
        if stop.linger.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::facing;
    use bevy::prelude::*;
    use bevy_sprinkles::prelude::ParticlesAsset;

    /// The effects are hand-edited RON; a typo would only show as a load error in the
    /// log and no effect.
    #[test]
    fn every_fx_in_assets_parses() {
        let mut checked = 0;
        for entry in std::fs::read_dir("assets/fx").expect("assets/fx exists") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().is_some_and(|e| e == "ron") {
                let text = std::fs::read_to_string(&path).expect("readable fx file");
                let asset: ParticlesAsset =
                    ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                if asset.emitters.iter().all(|e| e.time.one_shot) {
                    assert!(
                        asset.despawn_on_finish,
                        "{}: a one-shot that never despawns",
                        path.display()
                    );
                }
                checked += 1;
            }
        }
        assert!(
            checked >= 7,
            "expected every effect in assets/fx, found {checked}"
        );
    }

    #[test]
    fn facing_turns_up_to_the_direction() {
        let dir = Vec3::new(1.0, 1.0, 0.0).normalize();
        assert!((facing(dir) * Vec3::Y).distance(dir) < 1e-5);
        assert!((facing(Vec3::ZERO) * Vec3::Y).distance(Vec3::Y) < 1e-5);
    }
}
