use crate::game_state::GameState;
use crate::house_editor::canvas::{
    draw_canvas, handle_canvas_click, redraw_cells, spawn_house_editor_camera,
};
use crate::house_editor::state::HouseEditorState;
use crate::house_editor::ui::{
    handle_house_editor_keys, rebuild_house_panel, rebuild_info_label, spawn_house_editor_ui,
};
use crate::ui::spawn_ui::cleanup_state;
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{IntoScheduleConfigs, OnEnter, OnExit, in_state};

pub struct HouseEditorPlugin;

impl Plugin for HouseEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HouseEditorState>()
            .add_systems(
                OnEnter(GameState::HouseEditor),
                (spawn_house_editor_ui, spawn_house_editor_camera),
            )
            .add_systems(OnExit(GameState::HouseEditor), cleanup_state)
            .add_systems(
                Update,
                (
                    handle_house_editor_keys,
                    handle_canvas_click,
                    draw_canvas,
                    redraw_cells,
                    rebuild_info_label,
                    rebuild_house_panel,
                )
                    .run_if(in_state(GameState::HouseEditor)),
            );
    }
}
