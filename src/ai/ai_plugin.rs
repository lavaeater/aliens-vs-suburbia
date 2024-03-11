use crate::ai::components::move_towards_goal_components::{AgentCannotFindPath, AgentReachedGoal};
use crate::ai::systems::approach_and_attack_player_systems::{
    approach_and_attack_player_scorer_system, approach_player_action_system,
    attack_player_action_system, can_agent_see_player_system,
};
use crate::ai::systems::avoid_walls_systems::{
    avoid_walls_action_system, avoid_walls_data_system, avoid_walls_scorer_system,
};
use crate::ai::systems::destroy_the_map_systems::{
    agent_cant_find_path, destroy_the_map_action_system, destroy_the_map_scorer_system,
};
use crate::ai::systems::move_forward_systems::{
    move_forward_action_system, move_forward_scorer_system,
};
use crate::ai::systems::move_towards_goal_systems::{
    agent_reached_goal_handler, move_towards_goal_action_system, move_towards_goal_scorer_system,
};
use crate::game_state::GameState;
use bevy::app::{App, FixedUpdate, Plugin, PreUpdate, Update};
use bevy::prelude::{in_state, IntoSystemConfigs};
use big_brain::BigBrainPlugin;

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BigBrainPlugin::new(PreUpdate))
            .add_event::<AgentReachedGoal>()
            .add_event::<AgentCannotFindPath>()
            .add_systems(Update, (agent_reached_goal_handler, agent_cant_find_path))
            .add_systems(
                FixedUpdate,
                (avoid_walls_data_system, can_agent_see_player_system),
            )
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
                    destroy_the_map_action_system,
                ),
            );
    }
}

pub struct StatefulAiPlugin;

impl Plugin for StatefulAiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BigBrainPlugin::new(PreUpdate))
            .add_event::<AgentReachedGoal>()
            .add_event::<AgentCannotFindPath>()
            .add_systems(
                Update,
                (agent_reached_goal_handler, agent_cant_find_path)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                (avoid_walls_data_system, can_agent_see_player_system)
                    .run_if(in_state(GameState::InGame)),
            )
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
                    destroy_the_map_action_system,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
