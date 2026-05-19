use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs, OnEnter, OnExit};
use crate::game_state::GameState;
use crate::map_editor::grid::{handle_grid_click, rebuild_grid, spawn_grid_camera, update_hover_highlight};
use crate::map_editor::state::MapEditorState;
use crate::map_editor::ui::{handle_editor_keys, rebuild_palette, rebuild_wave_list, spawn_map_editor_ui};
use crate::ui::spawn_ui::cleanup_state;

pub struct MapEditorPlugin;

impl Plugin for MapEditorPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MapEditorState>()
            .add_systems(OnEnter(GameState::MapEditor), (spawn_map_editor_ui, spawn_grid_camera))
            .add_systems(OnExit(GameState::MapEditor), cleanup_state)
            .add_systems(
                Update,
                (
                    handle_editor_keys,
                    handle_grid_click,
                    update_hover_highlight,
                    rebuild_grid,
                    rebuild_palette,
                    rebuild_wave_list,
                ).run_if(in_state(GameState::MapEditor)),
            );
    }
}
