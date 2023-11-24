use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs};
use crate::game_state::GameState;
use crate::player::systems::spawn_players::{spawn_players};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Update,
                (
                    spawn_players,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}