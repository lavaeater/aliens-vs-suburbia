use std::time::{SystemTime, UNIX_EPOCH};
use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;
use crate::animation::animation_plugin::{AnimationStore, get_child_with_component_recursive};
use crate::game_state::GameState;
use crate::model_settings::resources::{ANIM_KEYS, DebugAnimSelection, ModelSettings, MODEL_SETTINGS_PATH};
use crate::player::components::Player;
use crate::player::systems::spawn_players::apply_model_settings_live;

pub struct ModelSettingsPlugin;

impl Plugin for ModelSettingsPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(ModelSettings::load())
            .insert_resource(DebugAnimSelection::default())
            .insert_resource(FileWatchTimer { last_mtime: mtime_of_settings(), timer: 0.0 })
            .add_systems(Update, (
                poll_model_settings_file,
                apply_model_settings_live,
                apply_debug_anim_selection,
            ).run_if(in_state(GameState::InGame)));
    }
}

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
        if let Ok(text) = std::fs::read_to_string(MODEL_SETTINGS_PATH) {
            if let Ok(loaded) = ron::from_str::<ModelSettings>(&text) {
                *model_settings = loaded;
            }
        }
    }
}

fn apply_debug_anim_selection(
    mut anim_sel: ResMut<DebugAnimSelection>,
    anim_store: Option<Res<AnimationStore>>,
    player_query: Query<Entity, With<Player>>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
) {
    if !anim_sel.dirty { return; }
    anim_sel.dirty = false;

    let Some(anim_store) = anim_store else { return; };
    let key = ANIM_KEYS[anim_sel.index % ANIM_KEYS.len()];

    for player_entity in player_query.iter() {
        if let Some(anim_entity) = get_child_with_component_recursive(player_entity, &child_query, &anim_player_query) {
            if let Ok(mut anim_player) = anim_player_query.get_mut(anim_entity) {
                if let Some(idx) = anim_store.anims.get("players").and_then(|m| m.get(&key)) {
                    anim_player.play(*idx).repeat();
                }
            }
        }
    }
}
