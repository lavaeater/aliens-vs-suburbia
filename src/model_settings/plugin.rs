use crate::animation::animation_plugin::{
    ANIM_KEYS, AnimationKey, AnimationStore, KeyNodes, get_child_with_component_recursive,
};
use crate::animation::body_mask;
use crate::animation::clips::ClipSet;
use crate::assets::asset_definition::AssetDefinition;
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::GameState;
use crate::model_settings::resources::{
    CharacterFolder, MODEL_SETTINGS_PATH, ModelSettings, PlayerAnimClips, scan_character_folder,
};
use crate::player::components::Player;
use crate::player::systems::spawn_players::{
    FixSceneTransform, WeaponsHidden, apply_model_settings_live,
};
use bevy::animation::AnimationTargetId;
use bevy::app::{App, Plugin, Update};
use bevy::gltf::{Gltf, GltfAssetLabel, GltfNode};
use bevy::prelude::IntoScheduleConfigs;
use bevy::prelude::*;
use bevy_mod_outline::AsyncWorldInheritOutline;
use std::time::UNIX_EPOCH;

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
        let default_def = settings
            .current_model_path(&char_folder)
            .and_then(|p| AssetDefinition::load(&p))
            .filter(|d| {
                matches!(
                    d.model_type,
                    crate::assets::asset_definition::ModelType::Player(_)
                )
            });

        app.insert_resource(settings)
            .insert_resource(char_folder)
            .insert_resource(PlayerAnimClips::default())
            .insert_resource(PlayerAssetDef(default_def))
            .insert_resource(FileWatchTimer {
                last_mtime: mtime_of_settings(),
                timer: 0.0,
            })
            .add_systems(
                Update,
                (
                    poll_model_settings_file,
                    scan_folder_on_change,
                    reload_player_model,
                    apply_model_settings_live,
                    sync_player_anim_clips,
                    build_player_anim_graph,
                )
                    .run_if(in_state(GameState::InGame)),
            );
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
        .map_or(0, |d| d.as_secs())
}

fn poll_model_settings_file(
    time: Res<Time>,
    mut watch: ResMut<FileWatchTimer>,
    mut model_settings: ResMut<ModelSettings>,
) {
    watch.timer += time.delta_secs();
    if watch.timer < 0.5 {
        return;
    }
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
    if !model_settings.is_changed() {
        return;
    }
    if model_settings.character_folder == *last_folder {
        return;
    }
    last_folder.clone_from(&model_settings.character_folder);
    char_folder.files = scan_character_folder(&model_settings.character_folder);
}

// ── Live model reload ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
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
    if !model_settings.is_changed() {
        return;
    }
    let Some(path) = model_settings.current_model_path(&char_folder) else {
        return;
    };
    if *last_path == Some(path.clone()) {
        return;
    }
    *last_path = Some(path.clone());

    // Reload the AssetDefinition whenever the model changes.
    // Only use it if the def's model_type is Player; otherwise clear so
    // the game falls back to WEAPON_NODES / default animation search.
    player_asset_def.0 = AssetDefinition::load(&path).filter(|d| {
        matches!(
            d.model_type,
            crate::assets::asset_definition::ModelType::Player(_)
        )
    });

    let scene: Handle<WorldAsset> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
    game_assets.player_scene = scene.clone();
    game_assets.player_gltf = asset_server.load(path);

    let s = &*model_settings;
    for player_entity in player_query.iter() {
        commands
            .entity(player_entity)
            .insert(WorldAssetRoot(scene.clone()))
            .insert(FixSceneTransform::new(
                Vec3::new(s.translation_x, s.translation_y, s.translation_z),
                Quat::from_rotation_y(s.rotation_y_degrees.to_radians()),
                Vec3::splat(s.scale),
            ))
            .remove::<WeaponsHidden>()
            .remove::<AsyncWorldInheritOutline>();
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
    let Some(gltf) = gltf_assets.get(&game_assets.player_gltf) else {
        return;
    };
    let id = game_assets.player_gltf.id();
    if *last_id == Some(id) && !clips.names.is_empty() {
        return;
    }
    *last_id = Some(id);

    let mut names: Vec<String> = gltf
        .named_animations
        .keys()
        .map(std::string::ToString::to_string)
        .collect();
    names.sort();
    clips.names = names;
}

// ── Player animation graph builder ───────────────────────────────────────────

/// Rebuilds the "players" entry in AnimationStore from the loaded GLTF.
/// Each key resolves through `ClipSet::resolve` (def, ModelSettings override, rig preset,
/// name search, fallback key).
#[allow(clippy::too_many_arguments)]
fn build_player_anim_graph(
    model_settings: Res<ModelSettings>,
    game_assets: Res<GameAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    asset_server: Res<AssetServer>,
    anim_store: Option<ResMut<AnimationStore>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    player_query: Query<
        (
            Entity,
            &crate::animation::animation_plugin::CurrentAnimationKey,
        ),
        With<Player>,
    >,
    child_query: Query<&Children>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
    mut commands: Commands,
    player_asset_def: Option<Res<PlayerAssetDef>>,
    mut last_sig: Local<String>,
) {
    let Some(gltf) = gltf_assets.get(&game_assets.player_gltf) else {
        return;
    };
    let Some(mut store) = anim_store else { return };

    let def = player_asset_def.as_ref().and_then(|r| r.0.as_ref());

    let def_sig = def.map_or_default(|d| {
        format!(
            "{:?}|{:?}",
            d.animations, d.animation_sources
        )
    });

    let sig = format!(
        "{}|{:?}|{}",
        game_assets.player_gltf.id(),
        model_settings.anim_mapping,
        def_sig
    );
    if *last_sig == sig {
        return;
    }

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

    let (graph, anims) = build_graph(
        def,
        Some(&model_settings.anim_mapping),
        gltf,
        &gltf_nodes,
        &extra_gltfs,
    );

    let graph_handle = animation_graphs.add(graph);
    store.anims.insert("players".to_string(), anims);
    store
        .graphs
        .insert("players".to_string(), graph_handle.clone());

    // Update AnimationGraphHandle + resume current anim on existing player entities.
    for (player_entity, anim_key) in player_query.iter() {
        let Some(anim_entity) =
            get_child_with_component_recursive(player_entity, &child_query, &anim_player_query)
        else {
            continue;
        };
        let Ok(mut anim_player) = anim_player_query.get_mut(anim_entity) else {
            continue;
        };
        // `try_insert`: a rebuild can land on the frame a character is being swapped out.
        commands
            .entity(anim_entity)
            .try_insert(AnimationGraphHandle(graph_handle.clone()));
        store.play("players", anim_key.key, &mut anim_player, 0.0);
    }
}

/// Build an animation graph for a model from its def, an optional `ModelSettings` mapping
/// override, the model's own GLTF and any loaded external animation sources: one node per
/// key that resolves to a clip (`ClipSet::resolve`). Shared by the player and the
/// def-driven enemies.
///
/// A key with an upper-body layer (`ClipSet::upper_body_layer`: walking while aiming on
/// a rig with only an aim pose) gets two nodes instead, its own clip with the upper body
/// masked out and the aim clip with the lower body masked out (`body_mask`).
pub fn build_graph(
    def: Option<&AssetDefinition>,
    settings_mapping: Option<&crate::model_settings::resources::AnimMapping>,
    gltf: &Gltf,
    gltf_nodes: &Assets<GltfNode>,
    extra_gltfs: &[(&String, &Gltf)],
) -> (
    AnimationGraph,
    std::collections::HashMap<AnimationKey, KeyNodes>,
) {
    // Every playable clip by the name `ClipSet` resolves to: the model's own clip names,
    // and `"<stem>|<clip>"` for each source's.
    let mut handles: std::collections::HashMap<String, Handle<AnimationClip>> = gltf
        .named_animations
        .iter()
        .map(|(name, handle)| (name.to_string(), handle.clone()))
        .collect();
    let mut sourced = Vec::new();
    for (path, ext_gltf) in extra_gltfs {
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        for (name, handle) in &ext_gltf.named_animations {
            handles
                .entry(format!("{stem}|{name}"))
                .or_insert_with(|| handle.clone());
            sourced.push((stem.to_string(), name.to_string()));
        }
    }
    let clips = ClipSet::new(
        gltf.named_animations.keys().map(ToString::to_string),
        sourced,
    );
    let animations = def.map(|d| &d.animations);

    let mut graph = AnimationGraph::new();
    let mut masked = false;
    let mut anims = std::collections::HashMap::new();
    for &key in ANIM_KEYS {
        let Some(resolved) = clips.resolve(key, animations, settings_mapping) else {
            continue;
        };
        let Some(handle) = handles.get(&resolved.clip) else {
            continue;
        };
        let aim = clips
            .upper_body_layer(key, animations, settings_mapping)
            .and_then(|clip| handles.get(&clip));
        if let Some(aim) = aim
            && (masked || add_body_mask_groups(&mut graph, gltf, gltf_nodes, def))
        {
            masked = true;
            let lower =
                graph.add_clip_with_mask(handle.clone(), 1 << body_mask::UPPER, 1.0, graph.root);
            let upper =
                graph.add_clip_with_mask(aim.clone(), 1 << body_mask::LOWER, 1.0, graph.root);
            anims.insert(key, vec![lower, upper]);
        } else {
            anims.insert(key, vec![graph.add_clip(handle.clone(), 1.0, graph.root)]);
        }
    }
    (graph, anims)
}

/// Put every node of the model in `body_mask::UPPER` or `LOWER`. `false` (and nothing
/// added) when the skeleton has no bone to split at.
fn add_body_mask_groups(
    graph: &mut AnimationGraph,
    gltf: &Gltf,
    gltf_nodes: &Assets<GltfNode>,
    def: Option<&AssetDefinition>,
) -> bool {
    let paths = node_paths(gltf, gltf_nodes);
    let aim_bone = def
        .and_then(|d| d.aim_bones.first())
        .map(|b| b.bone.as_str());
    let Some(split) = body_mask::split_bone(&paths, aim_bone) else {
        return false;
    };
    for path in &paths {
        let group = if body_mask::is_upper(path, split) {
            body_mask::UPPER
        } else {
            body_mask::LOWER
        };
        let names: Vec<Name> = path.iter().cloned().map(Name::new).collect();
        graph.add_target_to_mask_group(AnimationTargetId::from_names(names.iter()), group);
    }
    true
}

/// Every node's name path from its scene root, parents first: the names Bevy's glTF
/// loader builds each bone's `AnimationTargetId` from.
fn node_paths(gltf: &Gltf, gltf_nodes: &Assets<GltfNode>) -> Vec<Vec<String>> {
    fn walk(
        handle: &Handle<GltfNode>,
        prefix: &[String],
        gltf_nodes: &Assets<GltfNode>,
        out: &mut Vec<Vec<String>>,
    ) {
        let Some(node) = gltf_nodes.get(handle) else {
            return;
        };
        let mut path = prefix.to_vec();
        path.push(node.name.clone());
        out.push(path.clone());
        for child in &node.children {
            walk(child, &path, gltf_nodes, out);
        }
    }
    let children: std::collections::HashSet<_> = gltf
        .nodes
        .iter()
        .filter_map(|h| gltf_nodes.get(h))
        .flat_map(|node| node.children.iter().map(Handle::id))
        .collect();
    let mut out = Vec::new();
    for root in gltf.nodes.iter().filter(|h| !children.contains(&h.id())) {
        walk(root, &[], gltf_nodes, &mut out);
    }
    out
}
