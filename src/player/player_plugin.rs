use crate::game_state::GameState;
use crate::player::components::{WeaponsHidden, WEAPON_NODES};
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};
use bevy::prelude::*;
use bevy::scene::{SceneInstance, SceneRoot};
use bevy_mod_outline::{AsyncSceneInheritOutline, AutoGenerateOutlineNormalsPlugin, OutlinePlugin, OutlineVolume};

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
            .add_systems(Update, auto_outline_scenes)
            .add_systems(
                Update,
                (
                    spawn_players,
                    fix_scene_transform,
                    auto_aim,
                    hide_player_weapon_nodes,
                )
                .run_if(in_state(GameState::InGame)),
            );
    }
}

fn auto_outline_scenes(
    mut commands: Commands,
    query: Query<Entity, (With<SceneRoot>, Without<AsyncSceneInheritOutline>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).insert((
            OutlineVolume {
                visible: true,
                width: 4.0,
                colour: Color::BLACK,
            },
            AsyncSceneInheritOutline::default(),
        ));
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
                if WEAPON_NODES.contains(&name.as_str()) {
                    commands.entity(entity).insert(Visibility::Hidden);
                }
            }
        }
    }
}
