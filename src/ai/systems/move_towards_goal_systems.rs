use bevy::prelude::*;
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::math::Vector2;
use bevy_xpbd_3d::prelude::{LinearVelocity, SpatialQuery, SpatialQueryFilter};
use big_brain::actions::ActionState;
use big_brain::scorers::Score;
use big_brain::thinker::{ActionSpan, Actor};
use crate::ai::components::approach_and_attack_player_components::{ApproachAndAttackPlayerData, AttackPlayerAction};
use crate::ai::components::move_towards_goal_components::{MoveTowardsGoalAction, MoveTowardsGoalData, MoveTowardsGoalScore};
use crate::general::components::{Attack, Health, Layer};
use crate::enemy::components::general::{Alien, AlienSightShape};
use crate::general::components::map_components::{AlienGoal, CurrentTile};
use crate::general::resources::map_resources::MapGraph;
use crate::player::components::general::{ControlDirection, Controller, ControlRotation, Player};
use pathfinding::directed::astar::astar;
use pathfinding::num_traits::Signed;

pub fn not_in_use_right_now(
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
            SpatialQueryFilter::new().with_masks([Layer::Player]), // Query for players
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

pub fn move_towards_goal_scorer_system(
    _approach_player_data: Query<&MoveTowardsGoalData>,
    mut query: Query<(&Actor, &mut Score), With<MoveTowardsGoalScore>>,
    goal_query: Query<&Position, With<AlienGoal>>,
) {
    for (Actor(_actor), mut score) in query.iter_mut() {
        if let Ok(_goal_position) = goal_query.get_single() {
            score.set(0.9); // we always want to move towards the goal, mate!
        }
    }
}

pub fn move_towards_goal_action_system(
    map_graph: Res<MapGraph>,
    mut action_query: Query<(&Actor, &mut ActionState, &ActionSpan), With<MoveTowardsGoalAction>>,
    mut alien_query: Query<(&mut MoveTowardsGoalData, &mut Controller, &Position, &Rotation, &CurrentTile), With<Alien>>,
) {
    for (Actor(actor), mut action_state, span) in action_query.iter_mut() {
        let _guard = span.span().enter();

        match *action_state {
            ActionState::Requested => {
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                if let Ok((mut move_towards_goal_data,
                              mut controller,
                              alien_position,
                              alien_rotation,
                              alien_current_tile)
                ) = alien_query.get_mut(*actor)
                {
                    match &move_towards_goal_data.path {
                        None => {
                            // Get a path, set the path, return here later, eh?
                            let astar =
                                astar(
                                    &alien_current_tile.tile,
                                    |t| map_graph.grid.neighbours(*t).into_iter().map(|t| (t, 1)),
                                    |t| map_graph.grid.distance(*t, map_graph.goal),
                                    |t| *t == map_graph.goal);
                            match astar {
                                None => {
                                    *action_state = ActionState::Failure;
                                }
                                Some(path) => {
                                    move_towards_goal_data.path = Some(path.0);
                                }
                            }
                        }
                        Some(path) => {
                            if path.len() == 0 {
                                move_towards_goal_data.path = None;
                                *action_state = ActionState::Success;
                            } else {
                                let next_tile = path[0];

                                let next_tile_position =
                                    Vec2::new(
                                        (next_tile.0 as f32) * 2.0f32 - 1.0,
                                        (next_tile.1 as f32) * 2.0f32 - 1.0);
                                let alien_position =
                                    Vec2::new(
                                    (alien_current_tile.tile.0 as f32) * 2.0f32 - 1.0,
                                    (alien_current_tile.tile.1 as f32) * 2.0f32 - 1.0);
                                let direction = next_tile_position - alien_position;
                                let distance = direction.length();
                                if distance < 0.5 {
                                    move_towards_goal_data.path = Some(path[1..].to_vec());
                                    return;
                                }
                                let direction = direction.normalize();
                                let alien_direction = alien_rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
                                let angle = direction.angle_between(alien_direction.xz()).to_degrees();
                                controller.directions.clear();
                                controller.rotations.clear();
                                if angle.abs() > 5.0 {
                                    if angle.is_negative() {
                                        controller.rotations.insert(ControlRotation::Left);
                                    } else {
                                        controller.rotations.insert(ControlRotation::Right);
                                    }
                                }
                                controller.directions.insert(ControlDirection::Forward);

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
    mut alien_query: Query<(&ApproachAndAttackPlayerData, &mut Controller, &Position, &Attack), With<Alien>>,
    mut player_query: Query<(&mut Health, &Position), With<Player>>,
) {
    for (Actor(actor), mut action_state, span) in action_query.iter_mut() {
        let _guard = span.span().enter();

        match *action_state {
            // Action was just requested; it hasn't been seen before.
            ActionState::Requested => {
                debug!("Let's attack the player!");
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
                            debug!("We no longer see they player!");
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
                                debug!("We are close enough to attack!");
                                player_health.health -= alien_attack.damage_range;
                                debug!("Player health: {}", player_health.health);
                                *action_state = ActionState::Success;
                            } else {
                                debug!("Player moved out of range!");
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

