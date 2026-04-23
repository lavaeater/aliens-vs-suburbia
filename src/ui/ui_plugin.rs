use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Camera2d, Commands, OnEnter, OnExit};
use lava_ui_builder::LavaUiPlugin;
use crate::game_state::GameState;
use crate::ui::spawn_ui::{
    add_health_bar, cleanup_state, game_theme, goto_state_system, GotoState,
    spawn_menu, spawn_ui, sync_health_bars, update_hud, AddHealthBar,
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LavaUiPlugin)
            .insert_resource(game_theme())
            .add_message::<GotoState>()
            .add_message::<AddHealthBar>()
            .add_systems(OnEnter(GameState::InGame), spawn_ui)
            .add_systems(OnEnter(GameState::Menu), (spawn_ui_camera, spawn_menu))
            .add_systems(OnExit(GameState::Menu), cleanup_state)
            .add_systems(OnExit(GameState::InGame), cleanup_state)
            .add_systems(Update, (goto_state_system, add_health_bar, sync_health_bars, update_hud));
    }
}

pub fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d::default(),
        bevy::prelude::Camera { order: 1, ..Default::default() },
    ));
}
