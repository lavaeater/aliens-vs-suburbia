use bevy::prelude::*;
use avian3d::prelude::{LinearVelocity, Position, Rotation};
use crate::ai::components::move_towards_goal_components::{AgentCannotFindPath, AgentReachedGoal, MoveTowardsGoalData};
use crate::alien::components::general::{Alien, AlienCounter};
use crate::general::components::map_components::{AlienGoal, CurrentTile};
use crate::general::resources::map_resources::MapGraph;
use pathfinding::directed::astar::astar;
use crate::building::systems::ToWorldCoordinates;
use crate::control::components::{ControlDirection, CharacterControl, ControlRotation};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::systems::map_systems::TileDefinitions;

#[allow(clippy::type_complexity)]
pub fn move_towards_goal_system(
    map_graph: Res<MapGraph>,
    mut alien_query: Query<(Entity, &mut MoveTowardsGoalData, &mut CharacterControl, &Position, &Rotation, &CurrentTile, &LinearVelocity), With<Alien>>,
    mut alien_reached_goal_mw: MessageWriter<AgentReachedGoal>,
    mut cant_find_path_mw: MessageWriter<AgentCannotFindPath>,
    tile_definitions: Res<TileDefinitions>,
    goal_query: Query<(), With<AlienGoal>>,
) {
    if goal_query.is_empty() {
        info!("Has no goal");
        return;
    }

    for (entity,
         mut move_towards_goal_data,
         mut controller,
         alien_position,
         alien_rotation,
         alien_current_tile,
         _linear_velocity,
    ) in alien_query.iter_mut() {
        
        // if move_towards_goal_data.path.is_some() && linear_velocity.0.length() < 0.001 {
        //     info!("this little alien is standing still, i.e. has no path to goal right now.");
        //     move_towards_goal_data.path = None;
        //     continue;
        // }

        // info!("This little alien is supposed to be moving!");

        match &move_towards_goal_data.path.clone() {
            None => {
                info!("We have no path!");
                let astar_result = astar(
                    &alien_current_tile.tile,
                    |t| map_graph.path_finding_grid.neighbours(*t).into_iter().map(|t| (t, 1)),
                    |t| map_graph.path_finding_grid.distance(*t, map_graph.goal),
                    |t| *t == map_graph.goal,
                );
                match astar_result {
                    None => {
                        info!("Could not find a path right now");
                        cant_find_path_mw.write(AgentCannotFindPath(entity));
                    }
                    Some(path) => {
                        info!("Found path! {:?}", &path);
                        move_towards_goal_data.path = Some(path.0[1..].to_vec());
                    }
                }
            }
            Some(path) => {
                info!("Alien has path!");
                if path.is_empty() {
                    info!("Path is empty");
                    move_towards_goal_data.path = None;
                    alien_reached_goal_mw.write(AgentReachedGoal(entity));
                } else {
                    info!("Path has some steps left!");
                    let next_tile = path[0];
                    if map_graph.path_finding_grid.has_vertex(next_tile) {
                        info!("Found the next tile");
                        let next_tile_position = next_tile.to_world_coords(&tile_definitions).xz();
                        let alien_position_vector2 = alien_position.0.xz();
                        let alien_direction_vector2 = alien_rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0)).xz();
                        let alien_to_goal_direction = next_tile_position - alien_position_vector2;
                        let distance = alien_to_goal_direction.length();
                        if distance < 0.5 {
                            info!("Distance was sooo short, lets move to next!");
                            move_towards_goal_data.path = Some(path[1..].to_vec());
                            continue;
                        }

                        let angle = alien_direction_vector2.angle_to(alien_to_goal_direction).to_degrees();
                        info!(
                            "Steering: alien_pos={:.2?} tile_pos={:.2?} facing={:.2?} to_goal={:.2?} angle={:.1}deg dist={:.2}",
                            alien_position.0.xz(), next_tile_position,
                            alien_direction_vector2, alien_to_goal_direction,
                            angle, distance
                        );
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
                            if angle > 0.0 {
                                controller.rotations.insert(ControlRotation::Left);
                            } else {
                                controller.rotations.insert(ControlRotation::Right);
                            }
                        }
                        if angle.abs() < angle_forward_value {
                            info!("We can move forward!");
                            controller.directions.insert(ControlDirection::Forward);
                        }
                    } else {
                        info!("Couldn't find next tile on map!");
                        move_towards_goal_data.path = None;
                    }
                }
            }
        }
    }
}

pub fn agent_reached_goal_handler(
    mut alien_counter: ResMut<AlienCounter>,
    mut reached_goal_mr: MessageReader<AgentReachedGoal>,
    mut commands: Commands,
    mut game_tracking_mw: MessageWriter<GameTrackingEvent>,
) {
    for AgentReachedGoal(alien) in reached_goal_mr.read() {
        info!("this little alien reached its goal");
        alien_counter.count -= 1;
        commands.entity(*alien).despawn();
        game_tracking_mw.write(GameTrackingEvent::AlienReachedGoal);
    }
}
