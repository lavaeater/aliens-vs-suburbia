use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Commands, Component, Entity, in_state, IntoSystemConfigs, Query, Res, SceneSpawner, Without};
use bevy::scene::SceneInstance;
use bevy_mod_outline::{AutoGenerateOutlineNormalsPlugin, InheritOutlineBundle, OutlinePlugin};
use crate::game_state::GameState;
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::spawn_players::{spawn_players};

#[derive(Default)]
pub struct PlayerPlugin {
    pub with_debug: bool,
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        if self.with_debug {
            app
                .add_systems(Update, (debug_gizmos)
                    .run_if(in_state(GameState::InGame)));
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
                    auto_aim,
                ).run_if(in_state(GameState::InGame)),
            );
    }
}

#[derive(Component)]
pub struct OutlineDone;

fn fix_mod_outline(
    mut commands: Commands,
    scene_query: Query<(Entity, &SceneInstance), Without<OutlineDone>>,
    scene_manager: Res<SceneSpawner>,
) {
    for (scene_entity, scene) in scene_query.iter() {
        if scene_manager.instance_is_ready(**scene) {
            for entity in scene_manager.iter_instance_entities(**scene) {
                commands
                    .entity(entity)
                    .insert(InheritOutlineBundle::default());
            }
            commands.entity(scene_entity).insert(OutlineDone);
        }
    }
}