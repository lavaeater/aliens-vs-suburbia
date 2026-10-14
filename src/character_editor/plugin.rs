//! Wires the character editor into the app.
//!
//! Everything here is gated on [`in_editor`], and the four pieces of normal-match
//! behaviour the character editor replaces are gated on `in_normal_game` at their own
//! definition sites (map loading, the HUD, wave spawning, the win/lose transition).

use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;
use bevy::transform::TransformSystems;

use crate::alien::wave_manager::WaveManager;
use crate::character_editor::animation::AnimationEditor;
use crate::character_editor::debug::{
    EditorOverlays, bias_gizmos_over_mesh, draw_gait_gizmos, draw_player_overlays,
    reset_gizmo_bias, sync_physics_toggle,
};
use crate::character_editor::doc::{
    DocBar, EditorDoc, EditorFocus, PropDoc, WeaponDoc, adopt_worn_def, sync_doc_to_live,
    sync_weapon_doc,
};
use crate::character_editor::focus::{EditorInputFocus, update_game_input_block};
use crate::character_editor::hardpoints::{
    HardpointEditor, apply_grip_to_equipped_weapon, apply_weapon_def_to_equipped_weapon,
};
use crate::character_editor::inspect::{
    drive_orbit_camera, hold_player_still, hold_rest_pose, inspect_keys, inspecting,
    orbit_camera_active, rest_pose_held, switch_orbit_camera,
};
use crate::character_editor::library::{navigate_library, rebuild_library, sync_search_query};
use crate::character_editor::models::{EditorLibrary, swap_player_model};
use crate::character_editor::text_field::{
    FieldKey, FocusedField, blur_on_click_elsewhere, draw_fields, focus_on_click, sync_text_capture,
    type_into_field,
};
use crate::character_editor::prop::{
    despawn_prop, hide_player_for_prop, keep_studio_for_props, sync_prop,
};
use crate::character_editor::shell::{ActiveTab, spawn_editor_ui, sync_tabs, update_status_bar};
use crate::character_editor::state::in_editor;
use crate::character_editor::ui::{
    clear_editor_viewport, end_editor_session, rebuild_animation_panel, rebuild_debug_toggles,
    rebuild_doc_bar, rebuild_hardpoint_panel,
    rebuild_weapon_list, refresh_animation_panel_on_def_change, sync_editor_viewport,
};
use crate::game_state::GameState;
use crate::general::components::map_components::MapFile;
use crate::general::events::map_events::LoadMap;

/// The arena: a small walled room with nothing in it, for trying out the controls.
const ARENA_MAP: &str = "assets/maps/playground.ron";

pub struct CharacterEditorPlugin;

impl Plugin for CharacterEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FieldKey>()
            .init_resource::<FocusedField>()
            .add_observer(focus_on_click)
            .init_resource::<EditorDoc>()
            .init_resource::<ActiveTab>()
            .init_resource::<PropDoc>()
            .init_resource::<EditorFocus>()
            .init_resource::<WeaponDoc>()
            .init_resource::<DocBar>()
            .add_systems(
                OnEnter(GameState::InGame),
                (
                    load_arena_map,
                    silence_waves,
                    spawn_editor_ui,
                    spawn_settings_panels,
                    init_model_list,
                    bias_gizmos_over_mesh,
                )
                    .run_if(in_editor),
            )
            .add_systems(
                OnExit(GameState::InGame),
                (
                    clear_editor_viewport,
                    end_editor_session,
                    reset_gizmo_bias,
                    despawn_prop,
                )
                    .run_if(in_editor),
            )
            .add_systems(
                Update,
                (
                    sync_editor_viewport,
                    sync_tabs,
                    update_status_bar,
                    update_game_input_block,
                    rebuild_doc_bar,
                    rebuild_weapon_list,
                    sync_physics_toggle,
                    rebuild_debug_toggles,
                    draw_player_overlays,
                    draw_gait_gizmos,
                    rebuild_hardpoint_panel,
                    refresh_animation_panel_on_def_change,
                    rebuild_animation_panel,
                )
                    .run_if(in_state(GameState::InGame))
                    .run_if(in_editor),
            )
            // The doc pipeline, in order: a swap opens the new doc, the weapon doc follows the
            // character's `PlayerProps::weapon`, the live character follows the doc, and the
            // live weapon follows both. Chained so each step sees the one before it this
            // frame rather than next.
            .add_systems(
                Update,
                (
                    swap_player_model,
                    adopt_worn_def,
                    sync_weapon_doc,
                    sync_doc_to_live,
                    apply_grip_to_equipped_weapon,
                    apply_weapon_def_to_equipped_weapon,
                    keep_studio_for_props,
                    sync_prop,
                    hide_player_for_prop,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame))
                    .run_if(in_editor),
            )
            // The Library and its search box. Typing runs before the query is read, and the
            // query before the list is drawn, so a keystroke shows in the same frame.
            .add_systems(
                Update,
                (
                    blur_on_click_elsewhere,
                    type_into_field,
                    sync_text_capture,
                    draw_fields,
                    sync_search_query,
                    navigate_library,
                    rebuild_library,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame))
                    .run_if(in_editor),
            )
            // Inspect mode -- see `inspect.rs` for why each lands where it does.
            .add_systems(
                Update,
                (inspect_keys, switch_orbit_camera)
                    .chain()
                    .run_if(in_state(GameState::InGame))
                    .run_if(in_editor),
            )
            .add_systems(
                PreUpdate,
                hold_player_still
                    .after(crate::control::mouse_aim::mouse_aim)
                    .before(crate::general::systems::dynamic_movement_system::dynamic_movement)
                    .run_if(in_state(GameState::InGame))
                    .run_if(in_editor)
                    .run_if(inspecting),
            )
            .add_systems(
                PostUpdate,
                (
                    drive_orbit_camera.run_if(orbit_camera_active),
                    // Last word on the bones: after the twist and the arm and leg IK.
                    hold_rest_pose
                        .after(crate::player::systems::leg_ik::apply_leg_ik)
                        .run_if(rest_pose_held),
                )
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::InGame))
                    .run_if(in_editor),
            );
    }
}

/// Load the sandbox arena instead of `level_01`.
fn load_arena_map(mut load_map_mw: MessageWriter<LoadMap>) {
    let text = match std::fs::read_to_string(ARENA_MAP) {
        Ok(text) => text,
        Err(err) => {
            error!("character editor: cannot read {ARENA_MAP}: {err}");
            return;
        }
    };
    match ron::from_str::<MapFile>(&text) {
        Ok(map) => {
            load_map_mw.write(LoadMap { map });
        }
        Err(err) => error!("character editor: cannot parse {ARENA_MAP}: {err}"),
    }
}

/// The camera and model tweak panels the normal HUD carries (F1 / F2), plus the gait panel
/// (F10). They are self-contained and their update systems already run for the whole of
/// `InGame`, so the character editor just needs to spawn them — no duplicate sliders.
fn spawn_settings_panels(mut commands: Commands, theme: Res<lava_ui_builder::LavaTheme>) {
    crate::ui::spawn_ui::spawn_camera_panel(commands.reborrow(), &theme);
    crate::ui::spawn_ui::spawn_model_panel(commands.reborrow(), &theme);
    crate::ui::gait_panel::spawn_gait_panel(commands, &theme);
}

/// Scan `assets/defs` and the model folders fresh on every entry, so a def written by the
/// asset browser in between sessions shows up without a restart.
fn init_model_list(mut commands: Commands) {
    commands.insert_resource(EditorLibrary::fresh());
    commands.insert_resource(EditorOverlays::default());
    commands.insert_resource(HardpointEditor {
        ui_dirty: true,
        ..Default::default()
    });
    // Fresh docs every session: `sync_weapon_doc` compares paths, so a stale one from a
    // previous session would suppress the reload of an edited-then-abandoned def.
    commands.insert_resource(EditorDoc::default());
    commands.insert_resource(WeaponDoc::default());
    commands.insert_resource(DocBar::default());
    commands.insert_resource(FocusedField::default());
    commands.insert_resource(PropDoc::default());
    commands.insert_resource(EditorFocus::default());
    commands.insert_resource(EditorInputFocus::default());
    commands.insert_resource(crate::control::components::GameInputBlock::default());
    commands.insert_resource(ActiveTab::default());
    commands.insert_resource(AnimationEditor {
        ui_dirty: true,
        ..Default::default()
    });
}

/// `WaveManager::default()` ships a full set of hardcoded waves, and `map_loader` only
/// overrides them when the map file declares some — so an empty `waves: []` is not enough
/// to stop aliens pouring in. Clear them explicitly.
fn silence_waves(mut waves: ResMut<WaveManager>) {
    waves.waves.clear();
    waves.current_wave = 0;
    waves.spawning = false;
}
