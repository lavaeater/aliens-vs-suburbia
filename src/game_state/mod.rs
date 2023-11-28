pub mod game_state_plugin;
pub mod clear_game_entities_plugin;
pub mod score_keeper;

use bevy::prelude::States;

#[derive(Debug, Clone, Eq, PartialEq, Hash, States, Default)]
pub enum GameState {
    #[default]
    Menu,
    InGame,
}
