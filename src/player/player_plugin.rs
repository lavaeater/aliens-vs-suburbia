use crate::asset_browser::state::CHARACTER_NODE_PREFIX;
use crate::game_state::GameState;
use crate::player::components::{OutlineDone, WeaponsHidden};
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};
use bevy::prelude::*;
use bevy::scene::SceneInstance;
use bevy_mod_outline::{AutoGenerateOutlineNormalsPlugin, InheritOutline, OutlinePlugin};

#[derive(Default)]
pub struct PlayerPlugin {
    pub with_debug: bool,
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        if self.with_debug {
            app.add_systems(Update, debug_gizmos.run_if(in_state(GameState::InGame)));
        }
        app.add_plugins((OutlinePlugin, AutoGenerateOutlineNormalsPlugin::default()))
            .add_systems(
                Update,
                (
                    spawn_players,
                    setup_scene_once_loaded,
                    fix_scene_transform,
                    auto_aim,
                    hide_player_weapon_nodes,
                )
                .run_if(in_state(GameState::InGame)),
            );
    }
}

fn setup_scene_once_loaded(
    mut commands: Commands,
    scene_query: Query<(Entity, &SceneInstance), Without<OutlineDone>>,
    scene_manager: Res<SceneSpawner>,
) {
    for (scene_entity, scene) in scene_query.iter() {
        if scene_manager.instance_is_ready(**scene) {
            for entity in scene_manager.iter_instance_entities(**scene) {
                commands.entity(entity).insert(InheritOutline);
            }
            commands.entity(scene_entity).insert(OutlineDone);
        }
    }
}

/// After the player scene loads, hide all named mesh nodes whose name does not
/// start with "Character_" — those are weapons and other optional accessories.
fn hide_player_weapon_nodes(
    mut commands: Commands,
    player_query: Query<(Entity, &SceneInstance), (With<crate::player::components::Player>, Without<WeaponsHidden>)>,
    scene_spawner: Res<SceneSpawner>,
    named_mesh_query: Query<(Entity, &Name), With<Mesh3d>>,
) {
    for (player_entity, scene_instance) in player_query.iter() {
        if !scene_spawner.instance_is_ready(**scene_instance) { continue; }
        commands.entity(player_entity).insert(WeaponsHidden);
        for entity in scene_spawner.iter_instance_entities(**scene_instance) {
            if let Ok((_, name)) = named_mesh_query.get(entity) {
                if !name.starts_with(CHARACTER_NODE_PREFIX) {
                    commands.entity(entity).insert(Visibility::Hidden);
                }
            }
        }
    }
}
