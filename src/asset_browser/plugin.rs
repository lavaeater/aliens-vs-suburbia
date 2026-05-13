use bevy::app::{App, Plugin, Update};
use bevy::prelude::{IntoScheduleConfigs, OnEnter, OnExit, in_state};
use crate::asset_browser::state::AssetBrowserState;
use crate::asset_browser::ui::{handle_key_input, rebuild_list, scroll_to_selection, spawn_asset_browser_ui};
use crate::asset_browser::viewer::{
    handle_model_load, orbit_viewer, zoom_viewer, spawn_asset_browser_cameras, sync_viewer_viewport,
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
                    rebuild_list,
                    scroll_to_selection,
                    orbit_viewer,
                    zoom_viewer,
                    sync_viewer_viewport,
                )
                    .run_if(in_state(GameState::AssetBrowser)),
            );
    }
}
