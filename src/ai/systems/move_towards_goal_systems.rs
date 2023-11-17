use bevy::prelude::*;
use bevy_xpbd_3d::components::{Position, Rotation};
use big_brain::actions::ActionState;
use big_brain::scorers::Score;
use big_brain::thinker::{ActionSpan, Actor};
use crate::ai::components::move_towards_goal_components::{MoveTowardsGoalAction, MoveTowardsGoalData, MoveTowardsGoalScore};
use crate::enemy::components::general::{Alien, AlienCounter};
use crate::general::components::map_components::{AlienGoal, CurrentTile};
use crate::general::resources::map_resources::MapGraph;
use crate::player::components::general::{ControlDirection, Controller, ControlRotation};
use pathfinding::directed::astar::astar;
use pathfinding::num_traits::Signed;
use crate::general::events::map_events::AlienReachedGoal;

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
    mut alien_reached_goal_event_writer: EventWriter<AlienReachedGoal>,
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
                              alien_current_tile)
                ) = alien_query.get_mut(actor.0)
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
                                    move_towards_goal_data.path = Some(path.0[1..].to_vec());
                                }
                            }
                        }
                        Some(path) => {
                            if path.is_empty() {
                                move_towards_goal_data.path = None;
                                alien_reached_goal_event_writer.send(AlienReachedGoal(actor.0));
                                *action_state = ActionState::Success;
                            } else {
                                let next_tile = path[0];
                                if map_graph.grid.has_vertex(next_tile) {
                                    let next_tile_position =
                                        Vec2::new(
                                            (next_tile.0 as f32) * 2.0f32 + 0.5,
                                            (next_tile.1 as f32) * 2.0f32 + 0.5);
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

pub fn alien_reached_goal_handler(
    mut alien_counter: ResMut<AlienCounter>,
    mut reached_goal_event_reader: EventReader<AlienReachedGoal>,
    mut commands: Commands
) {
    for AlienReachedGoal(alien) in reached_goal_event_reader.read() {
        alien_counter.count -= 1;
        commands.entity(*alien).despawn_recursive();
    }
}

