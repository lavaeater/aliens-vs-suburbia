use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::hierarchy::BuildChildren;
use bevy::math::Vec3;
use bevy::prelude::{Commands, Entity, EventReader, Has, info, Query, Res, Transform, With};
use bevy::scene::SceneBundle;
use bevy::time::Time;
use bevy::utils::hashbrown::HashSet;
use bevy_xpbd_3d::components::{Collider, CollidingEntities, LinearVelocity, Position, RigidBody};
use bevy_xpbd_3d::prelude::{Collision, CollisionLayers, Sensor};
use big_brain::actions::{ActionState, Steps};
use big_brain::pickers::Highest;
use big_brain::scorers::Score;
use big_brain::thinker::{ActionSpan, Actor, Thinker, ThinkerBuilder};
use crate::enemy::components::general::Alien;
use crate::general::components::{Ball, CollisionLayer, Health, HittableTarget};
use crate::general::components::map_components::{CoolDown, CurrentTile, ModelDefinitions};
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::general::IsObstacle;
use crate::towers::components::{AlienInRangeScore, ShootAlienAction, ShootAlienData, TowerSensor, TowerShooter};
use crate::towers::events::BuildTower;

pub fn build_tower_system(
    mut build_tower_er: EventReader<BuildTower>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    model_defs: Res<ModelDefinitions>,
    tile_defs: Res<TileDefinitions>,
) {
    for build_tower in build_tower_er.read() {
        let model_def = model_defs.definitions.get(build_tower.model_definition_key).unwrap();
        let mut ec = commands.spawn((
            Name::from(model_def.name),
            IsObstacle {}, // let this be, for now!
            SceneBundle {
                scene: asset_server.load(model_def.file),
                ..Default::default()
            },
            model_def.rigid_body,
            tile_defs.create_collider(model_def.width, model_def.height, model_def.depth),
            Position::from(build_tower.position),
            model_def.create_collision_layers(),
            CurrentTile::default(),
            Health::default(),
        ));
        if build_tower.model_definition_key == "tower" {
            ec.with_children(|parent| {
                parent.spawn((
                    Name::from("Sensor"),
                    Collider::cylinder(0.5, 2.0),
                    CollisionLayers::new([CollisionLayer::Sensor], [CollisionLayer::Alien]),
                    Position::from(build_tower.position),
                    TowerSensor {},
                    TowerShooter::new(20.0),
                    Sensor,
                    create_thinker()
                ));
            });
        }
    }
}

pub fn alien_in_range_scorer_system(
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
                    let a_pos = alien_query.get(**a).unwrap();
                    let b_pos = alien_query.get(**b).unwrap();
                    let a_dist = (a_pos.0 - tower_position.0).length_squared();
                    let b_dist = (b_pos.0 - tower_position.0).length_squared();
                    a_dist.partial_cmp(&b_dist).unwrap()
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
                            CollisionLayers::new([CollisionLayer::Ball], [CollisionLayer::Impassable, CollisionLayer::Floor, CollisionLayer::Alien, CollisionLayer::Player, CollisionLayer::AlienSpawnPoint, CollisionLayer::AlienGoal]),
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