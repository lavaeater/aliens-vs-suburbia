//! Wires the playground into the app.
//!
//! Everything here is gated on [`in_playground`], and the four pieces of normal-match
//! behaviour the playground replaces are gated on `in_normal_game` at their own
//! definition sites (map loading, the HUD, wave spawning, the win/lose transition).

use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;

use crate::alien::wave_manager::WaveManager;
use crate::game_state::GameState;
use crate::general::components::map_components::MapFile;
use crate::general::events::map_events::LoadMap;
use crate::playground::dummies::{respawn_dummies, spawn_dummy_posts};
use crate::playground::debug::{
    bias_gizmos_over_mesh, draw_player_overlays, reset_gizmo_bias, sync_physics_toggle,
    PlaygroundDebug,
};
use crate::playground::animation::AnimationEditor;
use crate::playground::hardpoints::{apply_grip_to_equipped_weapon, HardpointEditor};
use crate::playground::models::{swap_player_model, PlaygroundModels};
use crate::playground::state::in_playground;
use crate::playground::ui::{
    clear_playground_viewport, end_playground_session, rebuild_debug_toggles,
    rebuild_animation_panel, rebuild_import_browser, rebuild_hardpoint_panel,
    rebuild_model_list, refresh_animation_panel_on_def_change, spawn_playground_ui,
    sync_playground_viewport,
};

const PLAYGROUND_MAP: &str = "assets/maps/playground.ron";

pub struct PlaygroundPlugin;

impl Plugin for PlaygroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                load_playground_map,
                silence_waves,
                spawn_playground_ui,
                spawn_settings_panels,
                spawn_dummy_posts,
                init_model_list,
                bias_gizmos_over_mesh,
            )
                .run_if(in_playground),
        )
        .add_systems(
            OnExit(GameState::InGame),
            (clear_playground_viewport, end_playground_session, reset_gizmo_bias)
                .run_if(in_playground),
        )
        .add_systems(
            Update,
            (
                sync_playground_viewport,
                respawn_dummies,
                swap_player_model,
                rebuild_model_list,
                rebuild_import_browser,
                sync_physics_toggle,
                rebuild_debug_toggles,
                draw_player_overlays,
                rebuild_hardpoint_panel,
                apply_grip_to_equipped_weapon,
                refresh_animation_panel_on_def_change,
                rebuild_animation_panel,
            )
                .run_if(in_state(GameState::InGame))
                .run_if(in_playground),
        );
    }
}

/// Load the sandbox arena instead of `level_01`.
fn load_playground_map(mut load_map_mw: MessageWriter<LoadMap>) {
    let text = match std::fs::read_to_string(PLAYGROUND_MAP) {
        Ok(text) => text,
        Err(err) => {
            error!("playground: cannot read {PLAYGROUND_MAP}: {err}");
            return;
        }
    };
    match ron::from_str::<MapFile>(&text) {
        Ok(map) => { load_map_mw.write(LoadMap { map }); }
        Err(err) => error!("playground: cannot parse {PLAYGROUND_MAP}: {err}"),
    }
}

/// The camera and model tweak panels the normal HUD carries, toggled with F1 / F2. They
/// are self-contained and their update systems already run for the whole of `InGame`, so
/// the playground just needs to spawn them — no duplicate sliders.
fn spawn_settings_panels(mut commands: Commands, theme: Res<lava_ui_builder::LavaTheme>) {
    crate::ui::spawn_ui::spawn_camera_panel(commands.reborrow(), &theme);
    crate::ui::spawn_ui::spawn_model_panel(commands, &theme);
}

/// Scan `assets/defs` and the model folders fresh on every entry, so a def written by the
/// asset browser in between sessions shows up without a restart.
fn init_model_list(mut commands: Commands) {
    commands.insert_resource(PlaygroundModels::fresh());
    commands.insert_resource(PlaygroundDebug::default());
    commands.insert_resource(HardpointEditor { ui_dirty: true, ..Default::default() });
    commands.insert_resource(AnimationEditor { ui_dirty: true, ..Default::default() });
}

/// `WaveManager::default()` ships a full set of hardcoded waves, and `map_loader` only
/// overrides them when the map file declares some — so an empty `waves: []` is not enough
/// to stop aliens pouring in. Clear them explicitly.
fn silence_waves(mut waves: ResMut<WaveManager>) {
    waves.waves.clear();
    waves.current_wave = 0;
    waves.spawning = false;
}





