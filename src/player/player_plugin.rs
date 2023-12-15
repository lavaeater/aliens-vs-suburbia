use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs};
use crate::game_state::GameState;
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};

pub struct PlayerPlugin {
    pub with_debug: bool,
}

impl Default for PlayerPlugin {
    fn default() -> Self {
        Self { with_debug: false }
    }
}


impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        if self.with_debug {
            app.add_systems(Update, (debug_gizmos).run_if(in_state(GameState::InGame)));
        }
        app
            .add_systems(
                Update,
                (
                    spawn_players,
                    fix_scene_transform,
                    auto_aim,

                    // fix_collider_transform,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}