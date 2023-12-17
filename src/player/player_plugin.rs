use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Commands, in_state, IntoSystemConfigs, Local, Query, Res, SceneSpawner};
use bevy::scene::SceneInstance;
use bevy_mod_outline::{AutoGenerateOutlineNormalsPlugin, InheritOutlineBundle, OutlinePlugin};
use crate::game_state::GameState;
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};

#[derive(Default)]
pub struct PlayerPlugin {
    pub with_debug: bool,
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        if self.with_debug {
            app.add_systems(Update, (debug_gizmos).run_if(in_state(GameState::InGame)));
        }
        app
            .add_plugins((
                OutlinePlugin,
                AutoGenerateOutlineNormalsPlugin,
            ))
            .add_systems(
                Update,
                (
                    spawn_players,
                    setup_scene_once_loaded,
                    fix_scene_transform,
                    auto_aim,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}

fn setup_scene_once_loaded(
    mut commands: Commands,
    scene_query: Query<&SceneInstance>,
    scene_manager: Res<SceneSpawner>,
    mut done: Local<bool>,
) {
    if !*done {
        if let Ok(scene) = scene_query.get_single() {
            if scene_manager.instance_is_ready(**scene) {
                for entity in scene_manager.iter_instance_entities(**scene) {
                    commands
                        .entity(entity)
                        .insert(InheritOutlineBundle::default());
                }
                *done = true;
            }
        }
    }
}