use std::time::UNIX_EPOCH;
use bevy::app::{App, Plugin, Update};
use bevy::gltf::{Gltf, GltfAssetLabel};
use bevy::prelude::*;
use crate::animation::animation_plugin::{
    AnimationStore, ANIM_KEYS,
    get_child_with_component_recursive,
};
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::GameState;
use crate::model_settings::resources::{
    CharacterFolder, DebugAnimSelection, ModelSettings, PlayerAnimClips,
    MODEL_SETTINGS_PATH, scan_character_folder,
};
use crate::player::components::{OutlineDone, Player};
use crate::player::systems::spawn_players::{
    FixSceneTransform, WeaponsHidden, apply_model_settings_live,
};

pub struct ModelSettingsPlugin;

impl Plugin for ModelSettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = ModelSettings::load();
        let files = scan_character_folder(&settings.character_folder);
        app
            .insert_resource(settings)
            .insert_resource(CharacterFolder { files })
            .insert_resource(PlayerAnimClips::default())
            .insert_resource(DebugAnimSelection::default())
            .insert_resource(FileWatchTimer {
                last_mtime: mtime_of_settings(),
                timer: 0.0,
            })
            .add_systems(Update, (
                poll_model_settings_file,
                scan_folder_on_change,
                reload_player_model,
                apply_model_settings_live,
                sync_player_anim_clips,
                build_player_anim_graph,
                apply_debug_anim_selection,
            ).run_if(in_state(GameState::InGame)));
    }
}

// ── File watch ────────────────────────────────────────────────────────────────

#[derive(Resource)]
struct FileWatchTimer {
    last_mtime: u64,
    timer: f32,
}

fn mtime_of_settings() -> u64 {
    std::fs::metadata(MODEL_SETTINGS_PATH)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn poll_model_settings_file(
    time: Res<Time>,
    mut watch: ResMut<FileWatchTimer>,
    mut model_settings: ResMut<ModelSettings>,
) {
    watch.timer += time.delta_secs();
    if watch.timer < 0.5 { return; }
    watch.timer = 0.0;

    let mtime = mtime_of_settings();
    if mtime != watch.last_mtime {
        watch.last_mtime = mtime;
        if let Ok(text) = std::fs::read_to_string(MODEL_SETTINGS_PATH)
            && let Ok(loaded) = ron::from_str::<ModelSettings>(&text)
        {
            *model_settings = loaded;
        }
    }
}

// ── Folder scan ───────────────────────────────────────────────────────────────

fn scan_folder_on_change(
    model_settings: Res<ModelSettings>,
    mut char_folder: ResMut<CharacterFolder>,
    mut last_folder: Local<String>,
) {
    if !model_settings.is_changed() { return; }
    if model_settings.character_folder == *last_folder { return; }
    *last_folder = model_settings.character_folder.clone();
    char_folder.files = scan_character_folder(&model_settings.character_folder);
}

// ── Live model reload ─────────────────────────────────────────────────────────

fn reload_player_model(
    model_settings: Res<ModelSettings>,
    asset_server: Res<AssetServer>,
    char_folder: Res<CharacterFolder>,
    mut game_assets: ResMut<GameAssets>,
    player_query: Query<Entity, With<Player>>,
    mut commands: Commands,
    mut last_path: Local<Option<String>>,
) {
    if !model_settings.is_changed() { return; }
    let Some(path) = model_settings.current_model_path(&char_folder) else { return };
    if *last_path == Some(path.clone()) { return; }
    *last_path = Some(path.clone());

    let scene: Handle<Scene> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
    game_assets.player_scene = scene.clone();
    game_assets.player_gltf = asset_server.load(path);

    let s = &*model_settings;
    for player_entity in player_query.iter() {
        commands.entity(player_entity)
            .insert(SceneRoot(scene.clone()))
            .insert(FixSceneTransform::new(
                Vec3::new(s.translation_x, s.translation_y, s.translation_z),
                Quat::from_rotation_y(s.rotation_y_degrees.to_radians()),
                Vec3::splat(s.scale),
            ))
            .remove::<WeaponsHidden>()
            .remove::<OutlineDone>();
    }
}

// ── Anim clip names ───────────────────────────────────────────────────────────

/// Populates PlayerAnimClips.names whenever the player GLTF (re)loads.
fn sync_player_anim_clips(
    game_assets: Res<GameAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    mut clips: ResMut<PlayerAnimClips>,
    mut last_id: Local<Option<bevy::asset::AssetId<Gltf>>>,
) {
    let Some(gltf) = gltf_assets.get(&game_assets.player_gltf) else { return };
    let id = game_assets.player_gltf.id();
    if *last_id == Some(id) && !clips.names.is_empty() { return; }
    *last_id = Some(id);

    let mut names: Vec<String> = gltf.named_animations.keys().map(|k| k.to_string()).collect();
    names.sort();
    clips.names = names;
}

// ── Player animation graph builder ───────────────────────────────────────────

/// Rebuilds the "players" entry in AnimationStore from the loaded GLTF and the
/// configured AnimMapping. Runs whenever the GLTF or mapping changes.
fn build_player_anim_graph(
    model_settings: Res<ModelSettings>,
    game_assets: Res<GameAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    anim_store: Option<ResMut<AnimationStore>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    player_query: Query<(Entity, &crate::animation::animation_plugin::CurrentAnimationKey), With<Player>>,
    child_query: Query<&Children>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
    mut commands: Commands,
    mut last_sig: Local<String>,
) {
    let Some(gltf) = gltf_assets.get(&game_assets.player_gltf) else { return };
    let Some(mut store) = anim_store else { return };

    let sig = format!(
        "{}|{:?}",
        game_assets.player_gltf.id(),
        model_settings.anim_mapping,
    );
    if *last_sig == sig { return; }
    *last_sig = sig;

    let mut graph = AnimationGraph::new();
    let mut anims = std::collections::HashMap::new();

    for &key in ANIM_KEYS {
        let mapped = model_settings.anim_mapping.get(key);
        if mapped.is_empty() { continue; }
        let mapped_lc = mapped.to_lowercase();
        if let Some(handle) = gltf.named_animations.iter()
            .find(|(name, _)| name.to_lowercase().contains(&mapped_lc))
            .map(|(_, h)| h.clone())
        {
            anims.insert(key, graph.add_clip(handle, 1.0, graph.root));
        }
    }

    let graph_handle = animation_graphs.add(graph);
    store.anims.insert("players".to_string(), anims);
    store.graphs.insert("players".to_string(), graph_handle.clone());

    // Update AnimationGraphHandle + resume current anim on existing player entities.
    for (player_entity, anim_key) in player_query.iter() {
        let Some(anim_entity) = get_child_with_component_recursive(
            player_entity, &child_query, &anim_player_query,
        ) else { continue };
        let Ok(mut anim_player) = anim_player_query.get_mut(anim_entity) else { continue };
        commands.entity(anim_entity).insert(AnimationGraphHandle(graph_handle.clone()));
        if let Some(&idx) = store.anims.get("players").and_then(|m| m.get(&anim_key.key)) {
            anim_player.play(idx).repeat();
        }
    }
}

// ── Debug anim selection ──────────────────────────────────────────────────────

fn apply_debug_anim_selection(
    mut anim_sel: ResMut<DebugAnimSelection>,
    anim_store: Option<Res<AnimationStore>>,
    player_query: Query<Entity, With<Player>>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
) {
    if !anim_sel.dirty { return; }
    anim_sel.dirty = false;

    let Some(anim_store) = anim_store else { return };
    let key = ANIM_KEYS[anim_sel.index % ANIM_KEYS.len()];

    for player_entity in player_query.iter() {
        let Some(anim_entity) = get_child_with_component_recursive(
            player_entity, &child_query, &anim_player_query,
        ) else { continue };
        let Ok(mut anim_player) = anim_player_query.get_mut(anim_entity) else { continue };
        if let Some(&idx) = anim_store.anims.get("players").and_then(|m| m.get(&key)) {
            anim_player.play(idx).repeat();
        }
    }
}
