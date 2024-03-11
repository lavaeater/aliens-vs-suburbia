use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Commands, Component, Entity, in_state, IntoSystemConfigs, Query, Res, SceneSpawner, Without};
use bevy::scene::SceneInstance;
use crate::game_state::GameState;
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::spawn_players::{model_is_ready, spawn_players};

#[derive(Default)]
pub struct PlayerPlugin {
    pub with_debug: bool,
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        if self.with_debug {
            app
                .add_systems(Update, (debug_gizmos)
                    .run_if(in_state(GameState::InGame)));
        }
        app
            .add_systems(
                Update,
                (
                    spawn_players,
                    model_is_ready,
                    auto_aim,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}