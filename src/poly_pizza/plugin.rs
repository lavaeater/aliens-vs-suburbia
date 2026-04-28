use bevy::app::{App, Plugin, Update};
use bevy::prelude::{IntoScheduleConfigs, OnEnter, OnExit, in_state};
use crate::game_state::GameState;
use crate::poly_pizza::async_bridge::{PolyPizzaConfig, spawn_api_thread};
use crate::poly_pizza::library::ModelLibrary;
use crate::poly_pizza::state::PolyPizzaState;
use crate::poly_pizza::ui::{
    handle_api_responses, handle_key_input, handle_search_submit, handle_user_search_submit,
    rebuild_results_ui, spawn_polypizza_screen, sync_viewer_viewport, update_animated_filter_button,
    update_attribution_label, update_save_button_label, update_search_label, update_status_label,
    update_tag_input_label, update_username_label,
};
use crate::poly_pizza::viewer::{
    handle_toon_toggle, handle_viewer_load, orbit_viewer, spawn_polypizza_cameras,
};
use crate::ui::spawn_ui::cleanup_state;

pub struct PolyPizzaPlugin;

impl Plugin for PolyPizzaPlugin {
    fn build(&self, app: &mut App) {
        dotenvy::dotenv().ok();
        let api_key = std::env::var("POLY_PIZZA_API_KEY").unwrap_or_default();
        let channels = spawn_api_thread(api_key.clone());

        app.insert_resource(PolyPizzaConfig { api_key })
            .insert_resource(channels)
            .insert_resource(ModelLibrary::load())
            .init_resource::<PolyPizzaState>()
            .add_systems(OnEnter(GameState::PolyPizza), (spawn_polypizza_screen, spawn_polypizza_cameras))
            .add_systems(OnExit(GameState::PolyPizza), cleanup_state)
            .add_systems(
                Update,
                (
                    handle_key_input,
                    handle_search_submit,
                    handle_user_search_submit,
                    handle_api_responses,
                    rebuild_results_ui,
                    handle_viewer_load,
                )
                    .run_if(in_state(GameState::PolyPizza)),
            )
            .add_systems(
                Update,
                (
                    orbit_viewer,
                    handle_toon_toggle,
                    sync_viewer_viewport,
                    update_search_label,
                    update_username_label,
                    update_status_label,
                    update_attribution_label,
                    update_animated_filter_button,
                    update_tag_input_label,
                    update_save_button_label,
                )
                    .run_if(in_state(GameState::PolyPizza)),
            );
    }
}
