use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs, With, World};
use bevy::window::PrimaryWindow;
use bevy_inspector_egui::bevy_egui::{EguiContext, EguiPlugin};
use bevy_inspector_egui::{DefaultInspectorConfigPlugin, egui};
use crate::camera::camera_components::GameCamera;
use crate::control::components::{CharacterControl, ControllerFlag};
use crate::game_state::GameState;
use crate::general::components::map_components::Floor;
use crate::player::components::Player;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(EguiPlugin)
            .add_plugins(DefaultInspectorConfigPlugin) // adds default options and `InspectorEguiImpl`s
            .register_type::<CharacterControl>()
            .register_type::<ControllerFlag>()
            .add_systems(Update, (inspector_ui).run_if(in_state(GameState::InGame)));
    }
}

fn inspector_ui(world: &mut World) {
    let Ok(egui_context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .get_single(world)
        else {
            return;
        };
    let mut egui_context = egui_context.clone();

    egui::Window::new("UI").show(egui_context.get_mut(), |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // // equivalent to `WorldInspectorPlugin`
            // bevy_inspector_egui::bevy_inspector::ui_for_world(world, ui);
            //
            // egui::CollapsingHeader::new("Materials").show(ui, |ui| {
            //     bevy_inspector_egui::bevy_inspector::ui_for_assets::<StandardMaterial>(world, ui);
            // });

            ui.heading("Entities of Interest");
            bevy_inspector_egui::bevy_inspector::ui_for_world_entities_filtered::<With<Player>>(world, ui, true);
            bevy_inspector_egui::bevy_inspector::ui_for_world_entities_filtered::<With<Floor>>(world, ui, true);
            bevy_inspector_egui::bevy_inspector::ui_for_world_entities_filtered::<With<GameCamera>>(world, ui, true);
        });
    });
}