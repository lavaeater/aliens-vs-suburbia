use bevy::app::{App, Plugin, Update};
use bevy::prelude::{IntoScheduleConfigs, OnEnter, OnExit, in_state};
use crate::asset_browser::state::AssetBrowserState;
use crate::asset_browser::ui::{handle_key_input, rebuild_folder_list, rebuild_list, rebuild_mapping_list, rebuild_node_list, rebuild_sources_list, rebuild_type_picker, scroll_to_selection, spawn_asset_browser_ui};
use crate::asset_browser::viewer::{
    apply_node_visibility, apply_viewer_animation, apply_viewer_scale, compute_model_height,
    handle_model_load, load_extra_animation_sources, merge_extra_anim_clips,
    orbit_viewer, setup_viewer_animation, spawn_asset_browser_cameras,
    sync_viewer_viewport, zoom_viewer,
};
use crate::game_state::GameState;
use crate::ui::spawn_ui::cleanup_state;

pub struct AssetBrowserPlugin;

impl Plugin for AssetBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetBrowserState>()
            .add_systems(
                OnEnter(GameState::AssetBrowser),
                (spawn_asset_browser_ui, spawn_asset_browser_cameras),
            )
            .add_systems(OnExit(GameState::AssetBrowser), cleanup_state)
            .add_systems(
                Update,
                (
                    handle_key_input,
                    handle_model_load,
                    rebuild_folder_list,
                    rebuild_list,
                    scroll_to_selection,
                    orbit_viewer,
                    zoom_viewer,
                    sync_viewer_viewport,
                    setup_viewer_animation,
                    apply_viewer_animation,
                    apply_node_visibility,
                    compute_model_height,
                    apply_viewer_scale,
                    load_extra_animation_sources,
                    merge_extra_anim_clips,
                    rebuild_node_list,
                    rebuild_mapping_list,
                    rebuild_sources_list,
                    rebuild_type_picker,
                )
                    .run_if(in_state(GameState::AssetBrowser)),
            );
    }
}
