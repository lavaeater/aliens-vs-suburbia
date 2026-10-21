pub mod clear_game_entities_plugin;
pub mod game_state_plugin;
pub mod score_keeper;

use bevy::prelude::{States, SubStates};

#[derive(Debug, Clone, Eq, PartialEq, Hash, States, Default)]
pub enum GameState {
    #[default]
    Menu,
    PlayerSetup,
    InGame,
    ModelShowcase,
    PolyPizza,
    AssetBrowser,
    MapEditor,
    HouseEditor,
}

/// Whether a match is paused, only present in `InGame`. `Paused` freezes `Time<Virtual>`
/// (so physics, animation and wave timers stop) and closes every player's input; the
/// pause menu (`ui::pause_menu`) lives in it.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, SubStates, Default)]
#[source(GameState = GameState::InGame)]
pub enum PauseState {
    #[default]
    Running,
    Paused,
}
