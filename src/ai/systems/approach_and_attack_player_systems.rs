use bevy::prelude::*;
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::math::Vector2;
use bevy_xpbd_3d::prelude::{SpatialQuery, SpatialQueryFilter};
use big_brain::actions::ActionState;
use big_brain::scorers::Score;
use big_brain::thinker::{ActionSpan, Actor};
use crate::ai::components::approach_and_attack_player_components::{ApproachAndAttackPlayerData, ApproachAndAttackPlayerScore, ApproachPlayerAction, AttackPlayerAction};
use crate::general::components::{Attack, CollisionLayer, Health};
use crate::alien::components::general::{Alien, AlienSightShape};
use crate::control::components::{ControlDirection, CharacterControl, ControlRotation};
use crate::player::components::Player;

pub fn can_agent_see_player_system(
    mut approach_player_query: Query<(&mut ApproachAndAttackPlayerData, &AlienSightShape, &Position, &Rotation)>,
    spatial_query: SpatialQuery,
) {
    for (mut alien_brain, sight_shape, position, rotation) in approach_player_query.iter_mut() {
        let direction = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));

        /*
        What do we do know?
        We create some kind of "brain" for this alien, this brain will contain facts about
        the world around it, like types of creates it wants to see and of course features
        of the environment like walls etc. Perhaps it can build a mental model of the world in the
        form of a graph? Yes. It can.
         */

        match spatial_query.cast_shape(
            &sight_shape.shape, // Shape to cast
            position.0, // Origin
            sight_shape.rotation, // Rotation of shape
            direction,// Direction
            sight_shape.range, // Maximum time of impact (travel distance)
            true,
            SpatialQueryFilter::new().with_masks([CollisionLayer::Player]), // Query for players
        ) {
            None => {
                alien_brain.seen_player = None;
            }
            Some(hit_data) => {
                alien_brain.seen_player = Some(hit_data.entity);
            }
        }
    }
}

pub fn approach_and_attack_player_scorer_system(
    approach_player_data: Query<&ApproachAndAttackPlayerData>,
    mut query: Query<(&Actor, &mut Score), With<ApproachAndAttackPlayerScore>>,
) {
    for (Actor(actor), mut score) in query.iter_mut() {
        if let Ok(avoid_wall_data) = approach_player_data.get(*actor) {
            match avoid_wall_data.seen_player {
                None => {
                    score.set(0.0);
                }
                Some(_) => {
                    score.set(0.91);
                }
            }
        }
    }
}

pub fn approach_player_action_system(
    mut action_query: Query<(&Actor, &mut ActionState, &ActionSpan), With<ApproachPlayerAction>>,
    mut alien_query: Query<(&ApproachAndAttackPlayerData, &mut CharacterControl, &Position, &Rotation), With<Alien>>,
    player_query: Query<&Position, With<Player>>,
) {
    for (Actor(actor), mut action_state, span) in action_query.iter_mut() {
        let _guard = span.span().enter();

        match *action_state {
            // Action was just requested; it hasn't been seen before.
            ActionState::Requested => {
                // We don't really need any initialization code here, since the queries are cheap enough.
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                if let Ok(
                    (approach_player_data, mut controller, alien_position, alien_rotation)
                ) = alien_query.get_mut(*actor)
                {
                    // Look up the actor's position.
                    match approach_player_data.seen_player {
                        None => {
                            *action_state = ActionState::Failure;
                        }
                        Some(player_entity) => {
                            let alien_direction_vector3 = alien_rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
                            let alien_direction_vector2 = Vector2::new(alien_direction_vector3.x, alien_direction_vector3.z);
                            let alien_position_vector2 = Vector2::new(alien_position.0.x, alien_position.0.z);
                            let player_position = player_query.get(player_entity).unwrap();
                            let player_position_vector2 = Vector2::new(
                                player_position.0.x,
                                player_position.0.z,
                            );
                            let alien_to_player_direction = (player_position_vector2 - alien_position_vector2).normalize();
                            let angle = alien_direction_vector2.angle_between(alien_to_player_direction).to_degrees();
                            controller.rotations.clear();
                            if angle.abs() < 15.0 {
                                controller.directions.insert(ControlDirection::Forward);
                            } else if angle > 0.0 {
                                controller.rotations.insert(ControlRotation::Right);
                            } else {
                                controller.rotations.insert(ControlRotation::Left);
                            }
                            let distance = (player_position_vector2 - alien_position_vector2).length();
                            if distance < approach_player_data.attack_distance {
                                *action_state = ActionState::Success;
                            }
                        }
                    }
                }
            }
            ActionState::Cancelled => {
                // Always treat cancellations, or we might keep doing this forever!
                // You don't need to terminate immediately, by the way, this is only a flag that
                // the cancellation has been requested. If the actor is balancing on a tightrope,
                // for instance, you may let them walk off before ending the action.
                *action_state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

pub fn attack_player_action_system(
    mut action_query: Query<(&Actor, &mut ActionState, &ActionSpan), With<AttackPlayerAction>>,
    mut alien_query: Query<(&ApproachAndAttackPlayerData, &mut CharacterControl, &Position, &Attack), With<Alien>>,
    mut player_query: Query<(&mut Health, &Position), With<Player>>,
) {
    for (Actor(actor), mut action_state, span) in action_query.iter_mut() {
        let _guard = span.span().enter();

        match *action_state {
            // Action was just requested; it hasn't been seen before.
            ActionState::Requested => {
                // We don't really need any initialization code here, since the queries are cheap enough.
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                if let Ok((attack_player_data, mut controller, alien_position, alien_attack))
                    = alien_query.get_mut(*actor)
                {
                    // Look up the actor's position.
                    match attack_player_data.seen_player {
                        None => {
                            *action_state = ActionState::Failure;
                        }
                        Some(player_entity) => {
                            let alien_position_vector2 = Vector2::new(alien_position.0.x, alien_position.0.z);
                            let (mut player_health, player_position) = player_query.get_mut(player_entity).unwrap();
                            let player_position_vector2 = Vector2::new(
                                player_position.0.x,
                                player_position.0.z,
                            );
                            controller.rotations.clear();

                            let distance = (player_position_vector2 - alien_position_vector2).length();
                            if distance < attack_player_data.attack_distance * 2.0 {
                                player_health.health -= alien_attack.damage_range;
                                *action_state = ActionState::Success;
                            } else {
                                *action_state = ActionState::Failure;
                            }
                        }
                    }
                }
            }
            ActionState::Cancelled => {
                // Always treat cancellations, or we might keep doing this forever!
                // You don't need to terminate immediately, by the way, this is only a flag that
                // the cancellation has been requested. If the actor is balancing on a tightrope,
                // for instance, you may let them walk off before ending the action.
                *action_state = ActionState::Failure;
            }
            _ => {}
        }
    }
}

