use bevy::prelude::*;
use bevy::utils::petgraph::matrix_graph::Zero;
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::prelude::LinearVelocity;
use big_brain::actions::ActionState;
use big_brain::scorers::Score;
use big_brain::thinker::{ActionSpan, Actor};
use crate::ai::components::move_towards_goal_components::{AgentCannotFindPath, AgentReachedGoal, MoveTowardsGoalAction, MoveTowardsGoalData, MoveTowardsGoalScore};
use crate::alien::components::general::{Alien, AlienCounter};
use crate::general::components::map_components::{AlienGoal, CurrentTile};
use crate::general::resources::map_resources::MapGraph;
use pathfinding::directed::astar::astar;
use pathfinding::num_traits::Signed;
use crate::building::systems::ToWorldCoordinates;
use crate::control::components::{ControlDirection, Controller, ControlRotation};
use crate::general::systems::map_systems::TileDefinitions;

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
    mut alien_query: Query<(&mut MoveTowardsGoalData, &mut Controller, &Position, &Rotation, &CurrentTile, &LinearVelocity), With<Alien>>,
    mut alien_reached_goal_event_writer: EventWriter<AgentReachedGoal>,
    mut cant_find_path_ew: EventWriter<AgentCannotFindPath>,
    tile_definitions: Res<TileDefinitions>,
) {
    for (actor, mut action_state, span) in action_query.iter_mut() {
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
                              alien_current_tile, linear_velocity)
                ) = alien_query.get_mut(actor.0)
                {
                    if linear_velocity.length().is_zero() {
                        info!("We are STANDING STILL!");
                        move_towards_goal_data.path = None;
                        *action_state = ActionState::Failure;
                        return;
                    }
                    match &move_towards_goal_data.path {
                        None => {
                            // Get a path, set the path, return here later, eh?
                            let astar =
                                astar(
                                    &alien_current_tile.tile,
                                    |t| map_graph.path_finding_grid.neighbours(*t).into_iter().map(|t| (t, 1)),
                                    |t| map_graph.path_finding_grid.distance(*t, map_graph.goal),
                                    |t| *t == map_graph.goal);
                            match astar {
                                None => {
                                    cant_find_path_ew.send(AgentCannotFindPath(actor.0));
                                    *action_state = ActionState::Failure;
                                }
                                Some(path) => {
                                    move_towards_goal_data.path = Some(path.0[1..].to_vec());
                                }
                            }
                        }
                        Some(path) => {
                            if path.is_empty() {
                                move_towards_goal_data.path = None;
                                alien_reached_goal_event_writer.send(AgentReachedGoal(actor.0));
                                *action_state = ActionState::Success;
                            } else {
                                let next_tile = path[0];
                                if map_graph.path_finding_grid.has_vertex(next_tile) {
                                    let next_tile_position = next_tile.to_world_coords(&tile_definitions).xz();
                                    let alien_position_vector2 = alien_position.0.xz();

                                    let alien_direction_vector2 = alien_rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0)).xz();
                                    let alien_to_goal_direction = next_tile_position - alien_position_vector2;
                                    let distance = alien_to_goal_direction.length();
                                    if distance < 0.5 {
                                        move_towards_goal_data.path = Some(path[1..].to_vec());
                                        return;
                                    }

                                    let angle = alien_direction_vector2.angle_between(alien_to_goal_direction).to_degrees();
                                    controller.rotations.clear();
                                    controller.directions.clear();
                                    let angle_speed_value = 90.0;
                                    let angle_forward_value = 15.0;
                                    if angle.abs() < angle_speed_value {
                                        controller.turn_speed = controller.max_turn_speed * (angle.abs() / angle_speed_value);
                                    } else {
                                        controller.turn_speed = controller.max_turn_speed;
                                    }
                                    if angle.abs() > 1.0 {
                                        if angle.is_positive() {
                                            controller.rotations.insert(ControlRotation::Right);
                                        } else {
                                            controller.rotations.insert(ControlRotation::Left);
                                        }
                                    }
                                    if angle.abs() < angle_forward_value {
                                        controller.directions.insert(ControlDirection::Forward);
                                    }
                                } else {
                                    move_towards_goal_data.path = None;
                                    *action_state = ActionState::Failure;
                                }
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

pub fn agent_reached_goal_handler(
    mut alien_counter: ResMut<AlienCounter>,
    mut reached_goal_event_reader: EventReader<AgentReachedGoal>,
    mut commands: Commands
) {
    for AgentReachedGoal(alien) in reached_goal_event_reader.read() {
        alien_counter.count -= 1;
        commands.entity(*alien).despawn_recursive();
    }
}

