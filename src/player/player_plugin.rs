use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs, OnEnter};
use crate::game_state::GameState;
use crate::player::systems::spawn_players::{animate_players, load_player_animations, spawn_players};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                OnEnter(GameState::InGame),
                load_player_animations,
            )
            .add_systems(
                Update,
                (
                    spawn_players,
                    animate_players,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}