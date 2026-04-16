use bevy::prelude::*;
use avian3d::prelude::{Position, Rotation};
use crate::ai::components::move_towards_goal_components::AgentCannotFindPath;
use crate::alien::components::general::Alien;
use crate::general::components::map_components::CurrentTile;
use crate::general::resources::map_resources::MapGraph;
use crate::player::components::IsObstacle;
use pathfinding::directed::astar::astar;
use crate::ai::components::destroy_the_map_components::{MustDestroyTheMap, MustDestroyTheMapState};
use crate::general::systems::map_systems::TileDefinitions;
use itertools::Itertools;
use crate::building::systems::ToWorldCoordinates;
use crate::control::components::{ControlDirection, CharacterControl, ControlRotation};
use crate::general::components::Health;

pub fn agent_cant_find_path(
    mut alien_cant_find_path_mr: MessageReader<AgentCannotFindPath>,
    mut commands: Commands,
) {
    for AgentCannotFindPath(alien) in alien_cant_find_path_mr.read() {
        if let Some(mut alien_commands) = commands.get_entity(*alien) {
            alien_commands.insert(MustDestroyTheMap::new());
        }
    }
}

pub fn destroy_the_map_action_system(
    mut commands: Commands,
    mut map_graph: ResMut<MapGraph>,
    mut alien_query: Query<(Entity, &mut MustDestroyTheMap, &mut CharacterControl, &Position, &Rotation, &CurrentTile), With<Alien>>,
    mut obstacle_query: Query<(&IsObstacle, &CurrentTile, &mut Health)>,
    tile_definitions: Res<TileDefinitions>,
) {
    for (entity,
         mut must_destroy_data,
         mut controller,
         alien_position,
         alien_rotation,
         alien_current_tile,
    ) in alien_query.iter_mut() {
        match must_destroy_data.state {
            MustDestroyTheMapState::NotStarted => {
                must_destroy_data.state = MustDestroyTheMapState::SearchingForThingToDestroy;
            }
            MustDestroyTheMapState::SearchingForThingToDestroy => {
                let mut potential_targets: Vec<(usize, usize)> =
                    obstacle_query
                        .iter()
                        .map(|(_, current_tile, _)| current_tile.tile)
                        .sorted_by(|a, b|
                            map_graph.path_finding_grid.distance(*b, alien_current_tile.tile)
                                .cmp(&map_graph.path_finding_grid.distance(*a, alien_current_tile.tile)))
                        .collect();

                if potential_targets.is_empty() {
                    must_destroy_data.state = MustDestroyTheMapState::Failed;
                    continue;
                }

                let mut try_this_one = potential_targets.pop().unwrap();
                map_graph.path_finding_grid.add_vertex(try_this_one);
                let mut need_path = true;

                while need_path {
                    match astar(
                        &alien_current_tile.tile,
                        |t| map_graph.path_finding_grid.neighbours(*t).into_iter().map(|t| (t, 1)),
                        |t| map_graph.path_finding_grid.distance(*t, try_this_one),
                        |t| *t == try_this_one) {
                        None => {
                            match potential_targets.pop() {
                                None => {
                                    map_graph.path_finding_grid.remove_vertex(try_this_one);
                                    must_destroy_data.state = MustDestroyTheMapState::Failed;
                                    need_path = false;
                                }
                                Some(target) => {
                                    map_graph.path_finding_grid.remove_vertex(try_this_one);
                                    try_this_one = target;
                                    map_graph.path_finding_grid.add_vertex(try_this_one);
                                }
                            }
                        }
                        Some(path) => {
                            must_destroy_data.target_tile = Some(try_this_one);
                            map_graph.path_finding_grid.remove_vertex(try_this_one);
                            must_destroy_data.path_of_destruction = Some(path.0[1..].to_vec());
                            need_path = false;
                            must_destroy_data.state = MustDestroyTheMapState::MovingTowardsThingToDestroy;
                        }
                    }
                }
            }
            MustDestroyTheMapState::MovingTowardsThingToDestroy => {
                match &must_destroy_data.path_of_destruction.clone() {
                    None => {
                        must_destroy_data.state = MustDestroyTheMapState::Failed;
                    }
                    Some(path) => {
                        if path.is_empty() {
                            must_destroy_data.path_of_destruction = None;
                            must_destroy_data.state = MustDestroyTheMapState::DestroyingThing;
                        } else {
                            let next_tile = path[0];
                            match &must_destroy_data.target_tile {
                                None => {
                                    must_destroy_data.state = MustDestroyTheMapState::Failed;
                                    must_destroy_data.path_of_destruction = None;
                                }
                                Some(target_tile) => {
                                    if *target_tile == next_tile {
                                        must_destroy_data.path_of_destruction = None;
                                        must_destroy_data.state = MustDestroyTheMapState::DestroyingThing;
                                    } else if map_graph.path_finding_grid.has_vertex(next_tile) {
                                        let next_tile_position = next_tile.to_world_coords(&tile_definitions).xz();
                                        let alien_position_vector2 = alien_position.0.xz();
                                        let alien_direction_vector2 = alien_rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0)).xz();
                                        let alien_to_goal_direction = next_tile_position - alien_position_vector2;
                                        let distance = alien_to_goal_direction.length();
                                        if distance < 0.25 {
                                            must_destroy_data.path_of_destruction = Some(path[1..].to_vec());
                                        } else {
                                            let angle = alien_direction_vector2.angle_to(alien_to_goal_direction).to_degrees();
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
                                                    controller.rotations.insert(ControlRotation::Right);
                                                } else {
                                                    controller.rotations.insert(ControlRotation::Left);
                                                }
                                            }
                                            if angle.abs() < angle_forward_value {
                                                controller.directions.insert(ControlDirection::Forward);
                                            }
                                        }
                                    } else {
                                        must_destroy_data.path_of_destruction = None;
                                        must_destroy_data.target_tile = None;
                                        must_destroy_data.state = MustDestroyTheMapState::Failed;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            MustDestroyTheMapState::DestroyingThing => {
                match &must_destroy_data.target_tile {
                    None => {
                        must_destroy_data.state = MustDestroyTheMapState::Failed;
                    }
                    Some(target_tile) => {
                        let target_tile = *target_tile;
                        let mut did_not_hit = true;
                        for (_, tower_tile, mut health) in obstacle_query.iter_mut() {
                            if tower_tile.tile == target_tile {
                                did_not_hit = false;
                                health.health -= 10;
                                if health.health <= 0 {
                                    map_graph.path_finding_grid.add_vertex(target_tile);
                                }
                                must_destroy_data.target_tile = None;
                                must_destroy_data.state = MustDestroyTheMapState::Finished;
                                break;
                            }
                        }
                        if did_not_hit {
                            must_destroy_data.state = MustDestroyTheMapState::Failed;
                            must_destroy_data.path_of_destruction = None;
                        }
                    }
                }
            }
            MustDestroyTheMapState::Finished => {
                commands.entity(entity).remove::<MustDestroyTheMap>();
            }
            MustDestroyTheMapState::Failed => {
                commands.entity(entity).remove::<MustDestroyTheMap>();
            }
        }
    }
}
