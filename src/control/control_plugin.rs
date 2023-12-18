use bevy::app::{App, Plugin, Update};
use bevy::prelude::{in_state, IntoSystemConfigs};
use crate::general::systems::dynamic_movement_system::dynamic_movement;
use crate::general::systems::kinematic_movement_system::kinematic_movement;
use crate::control::keyboard_input::{keyboard_input};
use crate::game_state::GameState;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                keyboard_input,
                dynamic_movement,
                kinematic_movement,
            ),
        );
    }
}

pub struct StatefulControlPlugin;

impl Plugin for StatefulControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (keyboard_input,
             dynamic_movement,
             kinematic_movement,
            ).run_if(in_state(GameState::InGame)),
        );
    }
}
