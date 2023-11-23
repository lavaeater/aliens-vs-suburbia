use belly::build::BellyPlugin;
use bevy::app::{App, Plugin, Update};
use bevy::prelude::{OnEnter, OnExit};
use crate::game_state::GameState;
use crate::ui::spawn_ui::{cleanup_menu, cleanup_ui, spawn_menu, spawn_ui};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(BellyPlugin)
            .add_systems(
                OnEnter(GameState::InGame),
                spawn_ui,
            )
            .add_systems(
                OnExit(GameState::InGame),
                cleanup_ui,
            )
            .add_systems(
                OnEnter(GameState::Menu),
                spawn_menu,
            )
            .add_systems(
                OnExit(GameState::Menu),
                cleanup_menu,
            )

        ;
    }
}
