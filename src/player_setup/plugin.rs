use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, Camera2d, Camera, IntoScheduleConfigs, IsDefaultUiCamera, OnEnter, OnExit};
use crate::game_state::GameState;
use crate::player_setup::state::{PlayerRoster, PlayerSetupState};
use crate::player_setup::ui::{handle_setup_input, rebuild_slot_labels, spawn_player_setup_ui};
use crate::ui::spawn_ui::{cleanup_state, StateMarker};

pub struct PlayerSetupPlugin;

impl Plugin for PlayerSetupPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<PlayerSetupState>()
            .init_resource::<PlayerRoster>()
            .add_systems(OnEnter(GameState::PlayerSetup), (spawn_player_setup_ui, spawn_setup_camera))
            .add_systems(OnExit(GameState::PlayerSetup), cleanup_state)
            .add_systems(
                Update,
                (handle_setup_input, rebuild_slot_labels).run_if(in_state(GameState::PlayerSetup)),
            );
    }
}

fn spawn_setup_camera(mut commands: bevy::prelude::Commands) {
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera { order: 1, ..Default::default() },
        StateMarker,
    ));
}
