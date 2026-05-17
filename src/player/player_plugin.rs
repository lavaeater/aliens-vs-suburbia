use crate::game_state::GameState;
use crate::general::components::map_components::Floor;
use crate::model_settings::plugin::PlayerAssetDef;
use crate::player::components::{WeaponsHidden, WEAPON_NODES};
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::death_revive::{detect_player_death, player_revive_system};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};
use bevy::prelude::*;
use bevy::scene::{SceneInstance, SceneRoot};
use bevy_mod_outline::{AsyncSceneInheritOutline, AutoGenerateOutlineNormalsPlugin, InheritOutline, OutlinePlugin, OutlineVolume};

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
            .add_systems(Update, (auto_outline_scenes, sync_outline_with_visibility))
            .add_systems(
                Update,
                (
                    spawn_players,
                    fix_scene_transform,
                    auto_aim,
                    hide_player_weapon_nodes,
                    detect_player_death,
                    player_revive_system,
                )
                .run_if(in_state(GameState::InGame)),
            );
    }
}

fn auto_outline_scenes(
    mut commands: Commands,
    query: Query<Entity, (With<SceneRoot>, Without<AsyncSceneInheritOutline>, Without<Floor>)>,
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

/// Keeps outline rendering in sync with Visibility.
///
/// Two races to handle:
/// 1. Visibility::Hidden set first, InheritOutline added later by AsyncSceneInheritOutline.
/// 2. InheritOutline already present, Visibility::Hidden set later.
///
/// When a weapon is later made visible again, re-insert InheritOutline alongside Visibility::Visible.
fn sync_outline_with_visibility(
    mut commands: Commands,
    mut volume_query: Query<
        (&Visibility, &mut OutlineVolume),
        Or<(Changed<Visibility>, Added<OutlineVolume>)>,
    >,
    // Race 1: InheritOutline just added to an already-hidden entity.
    added_query: Query<(Entity, &Visibility), Added<InheritOutline>>,
    // Race 2: Visibility changed on an entity that already has InheritOutline.
    changed_query: Query<(Entity, &Visibility), (With<InheritOutline>, Changed<Visibility>)>,
) {
    for (vis, mut outline) in volume_query.iter_mut() {
        if outline.visible != !matches!(vis, Visibility::Hidden) {
            outline.visible = !matches!(vis, Visibility::Hidden);
        }
    }
    for (entity, vis) in added_query.iter().chain(changed_query.iter()) {
        if matches!(vis, Visibility::Hidden) {
            commands.entity(entity).remove::<InheritOutline>();
        }
    }
}

/// After the player scene loads, hide all nodes listed in the AssetDefinition
/// for this model (falling back to the hardcoded WEAPON_NODES if no def exists).
fn hide_player_weapon_nodes(
    mut commands: Commands,
    player_query: Query<(Entity, &SceneInstance), (With<crate::player::components::Player>, Without<WeaponsHidden>)>,
    scene_spawner: Res<SceneSpawner>,
    named_query: Query<(Entity, &Name)>,
    player_asset_def: Option<Res<PlayerAssetDef>>,
) {
    // Build the effective hide list: prefer AssetDefinition, fall back to WEAPON_NODES.
    let def_nodes: Vec<&str>;
    let hidden: &[&str] = if let Some(def_res) = &player_asset_def
        && let Some(def) = &def_res.0
        && !def.hidden_nodes.is_empty()
    {
        def_nodes = def.hidden_nodes.iter().map(|s| s.as_str()).collect();
        &def_nodes
    } else {
        WEAPON_NODES
    };

    for (player_entity, scene_instance) in player_query.iter() {
        if !scene_spawner.instance_is_ready(**scene_instance) { continue; }
        commands.entity(player_entity).insert(WeaponsHidden);
        for entity in scene_spawner.iter_instance_entities(**scene_instance) {
            if let Ok((_, name)) = named_query.get(entity) {
                if hidden.contains(&name.as_str()) {
                    commands.entity(entity).insert(Visibility::Hidden);
                }
            }
        }
    }
}
