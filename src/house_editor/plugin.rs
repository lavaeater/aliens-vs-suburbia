use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs, OnEnter, OnExit};
use crate::game_state::GameState;
use crate::house_editor::canvas::{handle_canvas_click, redraw_canvas, spawn_house_editor_camera};
use crate::house_editor::state::HouseEditorState;
use crate::house_editor::ui::{handle_house_editor_keys, rebuild_info_label, spawn_house_editor_ui};
use crate::ui::spawn_ui::cleanup_state;

pub struct HouseEditorPlugin;

impl Plugin for HouseEditorPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<HouseEditorState>()
            .add_systems(OnEnter(GameState::HouseEditor), (spawn_house_editor_ui, spawn_house_editor_camera))
            .add_systems(OnExit(GameState::HouseEditor), cleanup_state)
            .add_systems(
                Update,
                (
                    handle_house_editor_keys,
                    handle_canvas_click,
                    redraw_canvas,
                    rebuild_info_label,
                ).run_if(in_state(GameState::HouseEditor)),
            );
    }
}
