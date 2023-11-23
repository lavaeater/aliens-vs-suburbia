use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::math::Vec3;
use bevy::prelude::{Commands, Query, Res, Transform, With};
use bevy::scene::SceneBundle;
use bevy::time::Time;
use bevy_xpbd_3d::components::{Collider, CollidingEntities, LinearVelocity, Position, RigidBody};
use bevy_xpbd_3d::prelude::CollisionLayers;
use big_brain::actions::{ActionState, Steps};
use big_brain::pickers::Highest;
use big_brain::scorers::Score;
use big_brain::thinker::{ActionSpan, Actor, Thinker, ThinkerBuilder};
use crate::alien::components::general::Alien;
use crate::general::components::{Ball, CollisionLayer};
use crate::general::components::map_components::CoolDown;
use crate::towers::components::{AlienInRangeScore, ShootAlienAction, TowerSensor, TowerShooter};

pub fn tower_has_alien_in_range_scorer_system(
    mut query: Query<(&Actor, &mut Score), With<AlienInRangeScore>>,
    colliding_entities_query: Query<&CollidingEntities>,
    alien_query: Query<&Alien>,
) {
    for (actor, mut score) in query.iter_mut() {
        if let Ok(colliding_entities) = colliding_entities_query.get(actor.0) {
            if colliding_entities.0.iter().any(|alien_entity| alien_query.contains(*alien_entity)) {
                score.set(1.0);
            } else {
                score.set(0.0);
            }
        }
    }
}

pub fn shoot_alien_system(
    mut commands: Commands,
    mut action_query: Query<(&Actor, &mut ActionState, &ActionSpan), With<ShootAlienAction>>,
    asset_server: Res<AssetServer>,
    mut tower_query: Query<(&Position, &CollidingEntities, &mut TowerShooter), With<TowerSensor>>,
    alien_query: Query<&Position, With<Alien>>,
    time: Res<Time>,
) {
    for (actor, mut action_state, _action_span) in action_query.iter_mut() {
        if let Ok((tower_position, colliding_entities, mut tower_shooter)) = tower_query.get_mut(actor.0) {
            if tower_shooter.cool_down(time.delta_seconds()) {
                let closes_alien = colliding_entities.0.iter().min_by(|a, b| {
                    if let (Ok(a_pos), Ok(b_pos)) = (alien_query.get(**a), alien_query.get(**b)) {
                        let a_dist = (a_pos.0 - tower_position.0).length_squared();
                        let b_dist = (b_pos.0 - tower_position.0).length_squared();
                        a_dist.partial_cmp(&b_dist).unwrap()
                    } else {
                        std::cmp::Ordering::Equal
                    }
                });
                match closes_alien {
                    None => {
                        *action_state = ActionState::Failure;
                        continue;
                    }
                    Some(alien_entity) => {
                        let alien_position = alien_query.get(*alien_entity).unwrap();
                        let direction = (alien_position.0 - tower_position.0).normalize();
                        let launch_p = tower_position.0 + direction + Vec3::new(0.0, 0.25, 0.0);

                        commands.spawn((
                            Name::from("Ball"),
                            Ball::default(),
                            SceneBundle {
                                scene: asset_server.load("ball_fab.glb#Scene0"),
                                transform: Transform::from_xyz(launch_p.x, launch_p.y, launch_p.z),
                                ..Default::default()
                            },
                            RigidBody::Dynamic,
                            Collider::ball(1.0 / 16.0),
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
                        ));
                        *action_state = ActionState::Success;
                    }
                }
            }
        }
    }
}

pub fn create_thinker() -> ThinkerBuilder {
    Thinker::build()
        .label("Tower Thinker")
        .picker(Highest {})
        .when(
            AlienInRangeScore,
            Steps::build()
                .label("Shoot Closest Alien")
                .step(ShootAlienAction),
        )
}