use bevy::app::{App, Plugin};
use crate::game_state::GameState;

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<GameState::Menu>();
    }
}