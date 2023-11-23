mod game_state_plugin;

use bevy::prelude::States;

#[derive(Debug, Clone, Eq, PartialEq, Hash, States, Default)]
enum GameState {
    #[default]
    Menu,
    InGame,
}
