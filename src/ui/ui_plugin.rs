use belly::build::BellyPlugin;
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Camera2dBundle, OnEnter, OnExit};
use crate::game_state::GameState;
use crate::ui::spawn_ui::{cleanup_menu, goto_state_system, GotoState, spawn_menu, spawn_ui};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<GotoState>()
            .add_plugins(BellyPlugin)
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
                goto_state_system,
            )
        ;
    }
}

pub fn spawn_ui_camera(
    mut commands: bevy::prelude::Commands,
) {
    commands.spawn(
        Camera2dBundle::default()
    );
}
