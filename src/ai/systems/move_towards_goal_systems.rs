use crate::ai::flow_field::Step;
use crate::ai::components::move_towards_goal_components::{
    AgentCannotFindPath, AgentReachedGoal, MoveTowardsGoalData,
};
use crate::alien::components::general::{Alien, AlienCounter};
use crate::building::systems::ToWorldCoordinates;
use crate::control::components::{CharacterControl, ControlDirection, ControlRotation};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::map_components::{AlienGoal, CurrentTile};
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::map_systems::TileDefinitions;
use crate::physics::prelude::{Position, Rotation};
use bevy::prelude::*;

/// Bring the flow field up to date before anyone reads it.
pub fn refresh_flow_field(mut map_graph: ResMut<MapGraph>) {
    if !map_graph.flow.fresh {
        map_graph.refresh_flow();
    }
}

/// Turn towards `target`, and walk once facing it.
fn steer(controller: &mut CharacterControl, position: Vec2, rotation: &Rotation, target: Vec2) {
    let facing = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0)).xz();
    let angle = facing.angle_to(target - position).to_degrees();

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
        controller.directions.insert(ControlDirection::Forward);
    }
}

#[allow(clippy::type_complexity)]
pub fn move_towards_goal_system(
    map_graph: Res<MapGraph>,
    mut alien_query: Query<
        (
            Entity,
            &mut MoveTowardsGoalData,
            &mut CharacterControl,
            &Position,
            &Rotation,
            &CurrentTile,
        ),
        With<Alien>,
    >,
    mut alien_reached_goal_mw: MessageWriter<AgentReachedGoal>,
    mut cant_find_path_mw: MessageWriter<AgentCannotFindPath>,
    tile_definitions: Res<TileDefinitions>,
    goal_query: Query<(), With<AlienGoal>>,
) {
    if goal_query.is_empty() {
        return;
    }

    for (entity, mut walk, mut controller, position, rotation, current_tile) in
        alien_query.iter_mut()
    {
        // Heading somewhere that has since been built on counts as heading nowhere.
        let held = walk
            .next
            .filter(|tile| map_graph.path_finding_grid.has_vertex(*tile));
        let next_tile = match held {
            Some(tile) => tile,
            None => match map_graph.flow.step_from(current_tile.tile) {
                Step::Towards(tile) => tile,
                Step::Arrived => {
                    walk.next = None;
                    alien_reached_goal_mw.write(AgentReachedGoal(entity));
                    continue;
                }
                Step::NoWay => {
                    walk.next = None;
                    cant_find_path_mw.write(AgentCannotFindPath(entity));
                    continue;
                }
            },
        };
        walk.next = Some(next_tile);

        let target = next_tile.to_world_coords(&tile_definitions).xz();
        if position.0.xz().distance(target) < 0.5 {
            walk.next = match map_graph.flow.step_from(next_tile) {
                Step::Towards(tile) => Some(tile),
                Step::Arrived | Step::NoWay => None,
            };
            continue;
        }
        steer(&mut controller, position.0.xz(), rotation, target);
    }
}

pub fn agent_reached_goal_handler(
    mut alien_counter: ResMut<AlienCounter>,
    mut reached_goal_mr: MessageReader<AgentReachedGoal>,
    mut commands: Commands,
    mut game_tracking_mw: MessageWriter<GameTrackingEvent>,
) {
    for AgentReachedGoal(alien) in reached_goal_mr.read() {
        trace!("this little alien reached its goal");
        alien_counter.count = alien_counter.count.saturating_sub(1);
        commands.entity(*alien).despawn();
        game_tracking_mw.write(GameTrackingEvent::AlienReachedGoal);
    }
}
