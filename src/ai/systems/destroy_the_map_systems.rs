use bevy::prelude::*;
use bevy::utils::petgraph::matrix_graph::Zero;
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::prelude::LinearVelocity;
use big_brain::actions::ActionState;
use big_brain::scorers::Score;
use big_brain::thinker::{ActionSpan, Actor};
use crate::ai::components::move_towards_goal_components::{AlienReachedGoal, CantFindPath, MoveTowardsGoalAction, MoveTowardsGoalData, MoveTowardsGoalScore};
use crate::enemy::components::general::{Alien, AlienCounter};
use crate::general::components::map_components::{AlienGoal, CurrentTile};
use crate::general::resources::map_resources::MapGraph;
use crate::player::components::general::{ControlDirection, Controller, ControlRotation, IsObstacle};
use pathfinding::directed::astar::astar;
use pathfinding::num_traits::Signed;
use crate::ai::components::destroy_the_map_components::{DestroyTheMapAction, DestroyTheMapScore, MustDestroyTheMap, MustDestroyTheMapState};
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::systems::build_systems::ToWorldCoordinates;

pub fn alien_cant_find_path(
    mut alien_cant_find_path_event_reader: EventReader<CantFindPath>,
    mut commands: Commands,
) {
    for CantFindPath(alien) in alien_cant_find_path_event_reader.read() {
        commands.entity(*alien).insert(MustDestroyTheMap::new());
    }
}

pub fn destroy_the_map_scorer_system(
    mut query: Query<(&Actor, &mut Score, Has<MustDestroyTheMap>), With<DestroyTheMapScore>>,
) {
    for (Actor(_actor), mut score, must_destroy) in query.iter_mut() {
        score.set(if must_destroy { 1.0 } else { 0.0 });
    }
}

pub fn destroy_the_map_action_system(
    map_graph: Res<MapGraph>,
    mut action_query: Query<(&Actor, &mut ActionState, &ActionSpan), With<DestroyTheMapAction>>,
    mut alien_query: Query<(&mut MustDestroyTheMap, &mut Controller, &Position, &Rotation, &CurrentTile, &LinearVelocity), With<Alien>>,
    obstacle_query: Query<(&IsObstacle, &CurrentTile)>,
    tile_definitions: Res<TileDefinitions>,
) {
    for (actor, mut action_state, span) in action_query.iter_mut() {
        let _guard = span.span().enter();

        match *action_state {
            ActionState::Requested => {
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                if let Ok((mut must_destroy_data,
                              mut controller,
                              alien_position,
                              alien_rotation,
                              alien_current_tile, linear_velocity)
                ) = alien_query.get_mut(actor.0)
                {
                    match must_destroy_data.state {
                        MustDestroyTheMapState::NotStarted => {
                            must_destroy_data.state = MustDestroyTheMapState::SearchingForThingToDestroy;
                        }
                        MustDestroyTheMapState::SearchingForThingToDestroy => {
                            /*
                            find a tower or obstacle that is reachable using the astar thingie
                             */
                            let mut need_path = true;
                            let mut potential_targets: Vec<(usize, usize)> =
                                obstacle_query
                                    .iter()
                                    .map(|(is_obstacle, current_tile)| current_tile.tile)
                                    .sort_by(|a, b|   )
                                    .collect()
                                    .into_iter()

                            let try_this_one = potential_targets.pop().unwrap();

                            while need_path {

                            }

                            let astar =
                                astar(
                                    &alien_current_tile.tile,
                                    |t| map_graph.path_finding_grid.neighbours(*t).into_iter().map(|t| (t, 1)),
                                    |t| map_graph.path_finding_grid.distance(*t, map_graph.goal),
                                    |t| *t == map_graph.goal);
                            match astar {
                                None => {
                                    *action_state = ActionState::Failure;
                                }
                                Some(path) => {
                                    move_towards_goal_data.path = Some(path.0[1..].to_vec());
                                }
                            }


                            must_destroy_data.state = MustDestroyTheMapState::MovingTowardsThingToDestroy;
                        }
                        MustDestroyTheMapState::MovingTowardsThingToDestroy => {

                            must_destroy_data.state = MustDestroyTheMapState::DestroyingThing;
                        }
                        MustDestroyTheMapState::DestroyingThing => {

                            must_destroy_data.state = MustDestroyTheMapState::Finished;
                        }
                        MustDestroyTheMapState::Finished => {
                            *action_state = ActionState::Success;
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

