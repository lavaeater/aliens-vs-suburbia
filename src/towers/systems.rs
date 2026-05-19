use bevy::prelude::Name;
use bevy::math::Vec3;
use bevy::prelude::{Commands, Entity, Query, Res, Transform, With};
use bevy::scene::SceneRoot;
use bevy::time::Time;
use avian3d::prelude::{Collider, CollidingEntities, CollisionLayers, LinearVelocity, Position, RigidBody};
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use crate::alien::components::general::Alien;
use crate::general::components::{Ball, CollisionLayer, Health};
use crate::general::components::map_components::CoolDown;
use crate::towers::components::{Slowed, TowerArea, TowerSensor, TowerShooter, TowerSlow};
use crate::assets::assets_plugin::GameAssets;

/// Applies a velocity penalty to aliens in a slow tower's sensor range.
/// The `Slowed` component acts as a TTL: refreshed every frame the alien is in
/// range, removed when it expires (alien left range and TTL decayed to zero).
pub fn slow_alien_system(
    mut commands: Commands,
    time: Res<Time>,
    sensor_query: Query<(&CollidingEntities, &TowerSlow), With<TowerSensor>>,
    mut alien_query: Query<(Entity, &mut LinearVelocity, Option<&mut Slowed>), With<Alien>>,
) {
    let dt = time.delta_secs();

    // Build a set of aliens currently in any slow zone.
    let mut in_range: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    let mut best_factor: std::collections::HashMap<Entity, f32> = std::collections::HashMap::new();
    for (colliding, slow) in sensor_query.iter() {
        for &e in colliding.iter() {
            if alien_query.contains(e) {
                in_range.insert(e);
                let entry = best_factor.entry(e).or_insert(1.0);
                *entry = entry.min(slow.factor);
            }
        }
    }

    for (entity, mut vel, slowed) in alien_query.iter_mut() {
        if let Some(factor) = best_factor.get(&entity) {
            // Scale velocity each frame the alien is in range.
            vel.0 *= *factor;
            if let Some(mut s) = slowed {
                s.factor = *factor;
                s.ttl = 0.15;
            } else {
                commands.entity(entity).insert(Slowed { factor: *factor, ttl: 0.15 });
            }
        } else if let Some(mut s) = slowed {
            s.ttl -= dt;
            if s.ttl <= 0.0 {
                commands.entity(entity).remove::<Slowed>();
            }
        }
    }
}

/// Deals flat damage per second to all aliens inside an area tower's sensor.
pub fn area_damage_system(
    mut sensor_query: Query<(&CollidingEntities, &mut TowerArea), With<TowerSensor>>,
    mut alien_query: Query<&mut Health, With<Alien>>,
    time: Res<Time>,
) {
    for (colliding, mut area) in sensor_query.iter_mut() {
        if !area.cool_down(time.delta_secs()) { continue; }
        let dmg = (area.damage_per_second * area.tick_interval) as i32;
        for &e in colliding.iter() {
            if let Ok(mut health) = alien_query.get_mut(e) {
                health.health -= dmg;
            }
        }
    }
}

pub fn shoot_alien_system(
    mut commands: Commands,
    mut tower_query: Query<(&Position, &CollidingEntities, &mut TowerShooter), With<TowerSensor>>,
    alien_query: Query<&Position, With<Alien>>,
    time: Res<Time>,
    game_assets: Res<GameAssets>,
) {
    for (tower_position, colliding_entities, mut tower_shooter) in tower_query.iter_mut() {
        // Check if any alien is in range
        let has_alien = colliding_entities.iter().any(|e| alien_query.contains(*e));
        if !has_alien {
            continue;
        }

        if tower_shooter.cool_down(time.delta_secs()) {
            let closest_alien = colliding_entities.iter().filter_map(|e| {
                alien_query.get(*e).ok().map(|pos| (*e, pos))
            }).min_by(|(_, a_pos), (_, b_pos)| {
                let a_dist = (a_pos.0 - tower_position.0).length_squared();
                let b_dist = (b_pos.0 - tower_position.0).length_squared();
                a_dist.partial_cmp(&b_dist).unwrap()
            });

            if let Some((_, alien_position)) = closest_alien {
                let direction = (alien_position.0 - tower_position.0).normalize();
                let launch_p = tower_position.0 + direction + Vec3::new(0.0, 0.25, 0.0);

                let entity = commands.spawn((
                    Name::from("Ball"),
                    SceneRoot(game_assets.ball_scene.clone()),
                    Transform::from_xyz(launch_p.x, launch_p.y, launch_p.z),
                    RigidBody::Dynamic,
                    Collider::sphere(1.0 / 16.0),
                    WindWakerShaderBuilder::default().build(),
                    LinearVelocity(direction * 12.0),
                    CollisionLayers::new(
                        [CollisionLayer::Ball],
                        [
                            CollisionLayer::Impassable,
                            CollisionLayer::Floor,
                            CollisionLayer::Alien,
                            CollisionLayer::Player,
                            CollisionLayer::AlienSpawnPoint,
                            CollisionLayer::AlienGoal
                        ]),
                )).id();

                commands.entity(entity).insert(Ball::new(entity));
            }
        }
    }
}
