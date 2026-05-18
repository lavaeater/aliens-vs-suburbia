use std::time::UNIX_EPOCH;
use bevy::app::{App, Plugin, Update};
use bevy::gltf::{Gltf, GltfAssetLabel};
use bevy::prelude::*;
use bevy::prelude::IntoScheduleConfigs;
use crate::animation::animation_plugin::{
    AnimationStore, ANIM_KEYS, clip_matches,
    get_child_with_component_recursive,
};
use crate::assets::asset_definition::AssetDefinition;
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::GameState;
use crate::model_settings::resources::{
    CharacterFolder, ModelSettings, PlayerAnimClips,
    MODEL_SETTINGS_PATH, scan_character_folder,
};
use crate::player::components::Player;
use bevy_mod_outline::AsyncSceneInheritOutline;
use crate::player::systems::spawn_players::{
    FixSceneTransform, WeaponsHidden, apply_model_settings_live,
};

/// Holds the `AssetDefinition` loaded for the current player model, if one
/// exists.  `None` means no `.ron` def was found; code falls back to defaults.
#[derive(Resource, Default)]
pub struct PlayerAssetDef(pub Option<AssetDefinition>);

pub struct ModelSettingsPlugin;

impl Plugin for ModelSettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = ModelSettings::load();
        let files = scan_character_folder(&settings.character_folder);
        // Load AssetDefinition for the default player model at startup.
        let char_folder = CharacterFolder { files };
        let default_def = settings.current_model_path(&char_folder)
            .and_then(|p| AssetDefinition::load(&p));

        app
            .insert_resource(settings)
            .insert_resource(char_folder)
            .insert_resource(PlayerAnimClips::default())
            .insert_resource(PlayerAssetDef(default_def))
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
    mut player_asset_def: ResMut<PlayerAssetDef>,
    player_query: Query<Entity, With<Player>>,
    mut commands: Commands,
    mut last_path: Local<Option<String>>,
) {
    if !model_settings.is_changed() { return; }
    let Some(path) = model_settings.current_model_path(&char_folder) else { return };
    if *last_path == Some(path.clone()) { return; }
    *last_path = Some(path.clone());

    // Reload the AssetDefinition whenever the model changes.
    player_asset_def.0 = AssetDefinition::load(&path);

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
            .remove::<AsyncSceneInheritOutline>();
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

/// Rebuilds the "players" entry in AnimationStore from the loaded GLTF.
/// Priority for each key: AssetDefinition mapping → ModelSettings.anim_mapping → default_search.
fn build_player_anim_graph(
    model_settings: Res<ModelSettings>,
    game_assets: Res<GameAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    asset_server: Res<AssetServer>,
    anim_store: Option<ResMut<AnimationStore>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    player_query: Query<(Entity, &crate::animation::animation_plugin::CurrentAnimationKey), With<Player>>,
    child_query: Query<&Children>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
    mut commands: Commands,
    player_asset_def: Option<Res<PlayerAssetDef>>,
    mut last_sig: Local<String>,
) {
    let Some(gltf) = gltf_assets.get(&game_assets.player_gltf) else { return };
    let Some(mut store) = anim_store else { return };

    let def = player_asset_def.as_ref().and_then(|r| r.0.as_ref());

    let def_sig = def
        .map(|d| format!("{:?}|{:?}", d.animation_mapping, d.animation_sources))
        .unwrap_or_default();

    let sig = format!("{}|{:?}|{}", game_assets.player_gltf.id(), model_settings.anim_mapping, def_sig);
    if *last_sig == sig { return; }

    // If any extra-source GltFs haven't loaded yet, wait before committing.
    let extra_gltfs: Vec<(&String, &Gltf)> = if let Some(d) = def {
        let mut loaded = Vec::new();
        for source_path in &d.animation_sources {
            let handle: Handle<Gltf> = asset_server.load(source_path.clone());
            if let Some(ext) = gltf_assets.get(&handle) {
                loaded.push((source_path, ext));
            } else {
                return; // wait for all sources to load
            }
        }
        loaded
    } else {
        Vec::new()
    };

    *last_sig = sig;

    let mut graph = AnimationGraph::new();
    let mut anims = std::collections::HashMap::new();

    for &key in ANIM_KEYS {
        // Determine the search value: AssetDefinition first, then ModelSettings, then default.
        let def_mapped = def
            .and_then(|d| d.animation_mapping.get(key.default_search()))
            .map(|s| s.as_str())
            .unwrap_or("");
        let settings_mapped = model_settings.anim_mapping.get(key);
        let search = if !def_mapped.is_empty() {
            def_mapped
        } else if !settings_mapped.is_empty() {
            settings_mapped
        } else {
            key.default_search()
        };

        // If the value contains '|', the part before it is a source file stem.
        let handle = if let Some(pipe) = search.find('|') {
            let stem = &search[..pipe];
            let clip_fragment = &search[pipe + 1..];
            // Find the matching extra GLTF by stem.
            extra_gltfs.iter()
                .find(|(path, _)| {
                    std::path::Path::new(path)
                        .file_stem().and_then(|s| s.to_str()) == Some(stem)
                })
                .and_then(|(_, ext_gltf)| {
                    ext_gltf.named_animations.iter()
                        .find(|(name, _)| {
                            let lower = name.to_lowercase();
                            let s = clip_fragment.to_lowercase();
                            let anim_part = lower.rsplit('|').next().unwrap_or(&lower);
                            anim_part == s || lower.ends_with(&s)
                        })
                        .or_else(|| ext_gltf.named_animations.iter()
                            .find(|(name, _)| clip_matches(name, clip_fragment)))
                        .map(|(_, h)| h.clone())
                })
        } else {
            // No pipe — search in the model's own GLTF as before.
            gltf.named_animations.iter()
                .find(|(name, _)| {
                    let lower = name.to_lowercase();
                    let s = search.to_lowercase();
                    let anim_part = lower.rsplit('|').next().unwrap_or(&lower);
                    anim_part == s || lower.ends_with(&s)
                })
                .or_else(|| gltf.named_animations.iter()
                    .find(|(name, _)| clip_matches(name, search)))
                .map(|(_, h)| h.clone())
        };

        if let Some(h) = handle {
            anims.insert(key, graph.add_clip(h, 1.0, graph.root));
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
            let active = anim_player.play(idx);
            if anim_key.key.loops() {
                active.repeat();
            }
        }
    }
}

