use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Camera2d, OnEnter, OnExit};
use crate::game_state::GameState;
use crate::ui::spawn_ui::{cleanup_menu, goto_state_system, GotoState, spawn_menu, spawn_ui,
                          add_health_bar, fellow_system, AddHealthBar};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<GotoState>()
            .add_message::<AddHealthBar>()
            .add_systems(
                OnEnter(GameState::InGame),
                spawn_ui,
            )
            .add_systems(
                OnEnter(GameState::Menu),
                (
                    spawn_ui_camera,
                    spawn_menu,
                ),
            )
            .add_systems(
                OnExit(GameState::Menu),
                cleanup_menu,
            )
            .add_systems(
                Update,
                (
                    goto_state_system,
                    add_health_bar,
                    fellow_system,
                ),
            );
    }
}

pub fn spawn_ui_camera(
    mut commands: bevy::prelude::Commands,
) {
    commands.spawn((Camera2d::default(), bevy::prelude::Camera { order: 1, ..Default::default() }));
}
