use bevy::app::{App, Plugin, PluginGroup, PluginGroupBuilder};
use crate::game_state::GameState;
use crate::map::map_plugins::StatefulMapPlugin;

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_state::<GameState>()
            .add_plugins(StatefulMapPlugin)
        ;
    }
}