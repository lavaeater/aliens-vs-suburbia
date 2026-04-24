use bevy::prelude::Name;
use bevy::math::Vec3;
use bevy::prelude::{Commands, Query, Res, Transform, With};
use bevy::scene::SceneRoot;
use bevy::time::Time;
use avian3d::prelude::{Collider, CollidingEntities, CollisionLayers, LinearVelocity, Position, RigidBody};
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use crate::alien::components::general::Alien;
use crate::general::components::{Ball, CollisionLayer};
use crate::general::components::map_components::CoolDown;
use crate::towers::components::{TowerSensor, TowerShooter};
use crate::assets::assets_plugin::GameAssets;

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

            if let Some((alien_entity, alien_position)) = closest_alien {
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
