use bevy::app::{App, FixedUpdate, Plugin, PreUpdate, Update};
use crate::ai::components::move_towards_goal_components::{AlienReachedGoal, CantFindPath};
use crate::ai::systems::approach_and_attack_player_systems::{approach_and_attack_player_scorer_system, approach_player_action_system, attack_player_action_system, can_i_see_player_system};
use crate::ai::systems::avoid_walls_systems::{avoid_walls_action_system, avoid_walls_data_system, avoid_walls_scorer_system};
use crate::ai::systems::destroy_the_map_systems::{alien_cant_find_path, destroy_the_map_action_system, destroy_the_map_scorer_system};
use crate::ai::systems::move_forward_systems::{move_forward_action_system, move_forward_scorer_system};
use crate::ai::systems::move_towards_goal_systems::{alien_reached_goal_handler, move_towards_goal_action_system, move_towards_goal_scorer_system};
use crate::enemy::systems::spawn_aliens::{alien_spawner_system, spawn_aliens};

pub struct AlienPlugin;

impl Plugin for AlienPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AlienReachedGoal>()
            .add_event::<CantFindPath>()

            .add_systems(
                Update,
                (
                    alien_spawner_system,
                    spawn_aliens,
                    alien_reached_goal_handler,
                    alien_cant_find_path,
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    avoid_walls_data_system,
                    can_i_see_player_system,
                ))
            .add_systems(
                PreUpdate,
                (
                    avoid_walls_scorer_system,
                    avoid_walls_action_system,
                    move_forward_scorer_system,
                    move_forward_action_system,
                    approach_and_attack_player_scorer_system,
                    approach_player_action_system,
                    attack_player_action_system,
                    move_towards_goal_scorer_system,
                    move_towards_goal_action_system,
                    destroy_the_map_scorer_system,
                    destroy_the_map_action_system
                ),
            );
    }
}
