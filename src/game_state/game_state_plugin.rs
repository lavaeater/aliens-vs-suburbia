use bevy::app::{App, Plugin};
use crate::game_state::GameState;
use crate::map::map_plugins::StatefulMapPlugin;
use crate::ui::spawn_ui::GotoState;
use crate::ui::ui_plugin::UiPlugin;

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_state::<GameState>()
            .add_event::<GotoState>()
            .add_plugins((
                StatefulMapPlugin,
                UiPlugin
            ))

        ;
    }
}