use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs};
use crate::game_state::GameState;
use crate::player::systems::auto_aim::{auto_aim, debug_auto_aim};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(
                Update,
                (
                    spawn_players,
                    fix_scene_transform,
                    auto_aim,
                    debug_auto_aim,
                    // fix_collider_transform,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}