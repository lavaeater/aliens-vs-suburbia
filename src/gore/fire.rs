//! Fire fields: a patch of burning ground that damages creatures standing in it over
//! time, throws up flames (a looping particle effect, `assets/fx/fire.ron`), and leaves
//! a scorch mark when it burns out.
//!
//! Driven by the [`SpawnFire`] message so any source can start one — the Molotov
//! ability now, thrown molotovs/other incendiaries later. Damage is emitted as
//! [`DamageDealt`] with [`DamageKind::Fire`], which the blood system skips (burns
//! don't spray) while still feeding death/score.

use avian3d::prelude::Position;
use bevy::prelude::*;
use bevy_sprinkles::prelude::Particles3d;

use crate::alien::components::general::Alien;
use crate::fx::{FxAssets, StopFx};
use crate::general::components::Health;
use crate::general::damage::ApplyDamage;
use crate::general::systems::map_systems::TileDefinitions;
use crate::gore::components::{DECAL_LIFT, DamageKind, GoreBudget};
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
    dps: f32,
    radius: f32,
    /// The flames' particle system, its own entity so it can outlive the field by
    /// the time its last flames take to die out.
    flames: Option<Entity>,
}

impl FireField {
    /// How often the field applies damage (also the window each tick's damage covers).
    const DAMAGE_INTERVAL: f32 = 0.25;
    /// How long the flames keep going after the field burns out: the longest particle
    /// lifetime in `fire.ron` (embers).
    const FLAME_LINGER: f32 = 1.2;

    pub fn new(duration: f32, dps: f32, radius: f32) -> Self {
        Self {
            life: Timer::from_seconds(duration, TimerMode::Once),
            damage_tick: Timer::from_seconds(Self::DAMAGE_INTERVAL, TimerMode::Repeating),
            dps,
            radius,
            flames: None,
        }
    }
}

/// Shared fire art.
#[derive(Resource)]
pub struct FireAssets {
    scorch_mesh: Handle<Mesh>,
    scorch_material: Handle<StandardMaterial>,
}

pub fn setup_fire_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scorch_mesh = meshes.add(Mesh::from(Plane3d::new(Vec3::Y, Vec2::splat(0.5))));
    let scorch_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.05, 0.04, 0.03, 0.9),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        depth_bias: 1.0,
        ..default()
    });
    commands.insert_resource(FireAssets {
        scorch_mesh,
        scorch_material,
    });
}

/// Spawn a `FireField` entity for each `SpawnFire`, with a glowing base blob and the
/// flames, stretched in X/Z to the field's radius.
pub fn spawn_fire_fields(
    mut msgs: MessageReader<SpawnFire>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    fx: Option<Res<FxAssets>>,
) {
    for fire in msgs.read() {
        let mut field = FireField::new(fire.duration, fire.dps, fire.radius);
        field.flames = fx.as_ref().map(|fx| {
            commands
                .spawn((
                    Name::new("Flames"),
                    Particles3d(fx.fire.clone()),
                    Transform::from_translation(fire.position)
                        .with_scale(Vec3::new(fire.radius, 1.0, fire.radius)),
                ))
                .id()
        });
        let base_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.35, 0.05, 0.5),
            emissive: LinearRgba::new(5.0, 1.6, 0.2, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            field,
            Mesh3d(meshes.add(Mesh::from(Sphere::new(0.5)))),
            MeshMaterial3d(base_mat),
            Transform::from_translation(fire.position + Vec3::Y * 0.15)
                .with_scale(Vec3::splat(fire.radius * 1.4)),
        ));
    }
}

/// Burn things in the fire, and on burnout scorch the ground and let the flames die.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tick_fire_fields(
    time: Res<Time>,
    mut commands: Commands,
    mut budget: ResMut<GoreBudget>,
    fire_assets: Option<Res<FireAssets>>,
    mut fields: Query<(Entity, &mut FireField, &Transform)>,
    targets: Query<(Entity, &Position, Has<Alien>, Has<Player>), With<Health>>,
    mut damage_mw: MessageWriter<ApplyDamage>,
    tile_defs: Res<TileDefinitions>,
) {
    let dt = time.delta();
    for (entity, mut field, transform) in fields.iter_mut() {
        field.life.tick(dt);
        field.damage_tick.tick(dt);

        let center = transform.translation;

        // ── Burn creatures inside the radius on each damage tick. ──────────────
        if field.damage_tick.just_finished() {
            let amount = (field.dps * field.damage_tick.duration().as_secs_f32()) as i32;
            if amount > 0 {
                let r2 = field.radius * field.radius;
                for (target, pos, is_alien, is_player) in targets.iter() {
                    if !(is_alien || is_player) {
                        continue;
                    }
                    if pos.0.distance_squared(center) <= r2 {
                        damage_mw.write(ApplyDamage::at(
                            target,
                            amount,
                            DamageKind::Fire,
                            pos.0 + Vec3::Y * 0.4,
                        ));
                    }
                }
            }
        }

        // ── Burn out: scorch the ground, then despawn. ─────────────────────────
        if field.life.is_finished() {
            if let Some(flames) = field.flames {
                commands
                    .entity(flames)
                    .insert(StopFx::after(FireField::FLAME_LINGER));
            }
            if let Some(fa) = fire_assets.as_ref() {
                let scorch = commands
                    .spawn((
                        Mesh3d(fa.scorch_mesh.clone()),
                        MeshMaterial3d(fa.scorch_material.clone()),
                        Transform::from_xyz(center.x, tile_defs.floor_level + DECAL_LIFT, center.z)
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

#[cfg(test)]
mod tests {
    use super::{FireField, tick_fire_fields};
    use crate::alien::components::general::Alien;
    use crate::game_state::score_keeper::GameTrackingEvent;
    use crate::general::components::Health;
    use crate::general::damage::{ApplyDamage, DamageRules, apply_damage};
    use crate::general::systems::map_systems::TileDefinitions;
    use crate::gore::components::{DamageDealt, DamageKind, GoreBudget};
    use avian3d::prelude::Position;
    use bevy::prelude::*;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Caught(Vec<DamageDealt>);

    fn catch(mut r: MessageReader<DamageDealt>, mut c: ResMut<Caught>) {
        for d in r.read() {
            c.0.push(*d);
        }
    }

    #[test]
    fn fire_burns_creatures_inside_the_radius_only() {
        let mut app = App::new();
        app.add_message::<DamageDealt>();
        app.add_message::<ApplyDamage>();
        app.add_message::<GameTrackingEvent>();
        app.init_resource::<DamageRules>();
        app.init_resource::<Caught>();
        app.init_resource::<Time>();
        app.init_resource::<GoreBudget>();
        // Scorch marks are placed on the floor plane, so the tile metrics have to exist.
        app.insert_resource(TileDefinitions::new(1.0, 32.0, 9.5, 1.0));
        // No FireAssets / FxAssets -> no scorch, no flames; the burning is what's tested.
        app.add_systems(Update, (tick_fire_fields, apply_damage, catch).chain());

        // Fire at the origin: radius 2, 40 dps.
        app.world_mut().spawn((
            FireField::new(5.0, 40.0, 2.0),
            Transform::from_translation(Vec3::ZERO),
        ));
        let inside = app
            .world_mut()
            .spawn((
                Alien,
                Health {
                    health: 100,
                    max_health: 100,
                },
                Position(Vec3::new(1.0, 0.0, 0.0)),
            ))
            .id();
        let outside = app
            .world_mut()
            .spawn((
                Alien,
                Health {
                    health: 100,
                    max_health: 100,
                },
                Position(Vec3::new(9.0, 0.0, 0.0)),
            ))
            .id();

        // 0.3s -> one 0.25s damage tick -> 40 * 0.25 = 10 damage.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(300));
        app.update();

        assert_eq!(
            app.world().get::<Health>(inside).unwrap().health,
            90,
            "creature in the fire burns"
        );
        assert_eq!(
            app.world().get::<Health>(outside).unwrap().health,
            100,
            "creature outside is safe"
        );

        let caught = &app.world().resource::<Caught>().0;
        assert_eq!(caught.len(), 1);
        assert_eq!(caught[0].target, inside);
        assert_eq!(caught[0].kind, DamageKind::Fire);
    }
}
