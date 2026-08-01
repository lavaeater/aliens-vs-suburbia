use bevy::app::{App, Plugin, PreUpdate, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs};
use crate::general::systems::dynamic_movement_system::{dynamic_movement_gamepad, dynamic_movement_keyboard};
use crate::general::systems::kinematic_movement_system::kinematic_movement;
use crate::control::gamepad_input::gamepad_game_input;
use crate::control::keyboard_input::{keyboard_input};
use crate::control::mouse_aim::{mouse_aim, mouse_face};
use crate::game_state::GameState;

#[allow(dead_code)]
pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                keyboard_input,
                dynamic_movement_keyboard,
                dynamic_movement_gamepad,
                kinematic_movement,
            ),
        );
    }
}

pub struct StatefulControlPlugin;

impl Plugin for StatefulControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (keyboard_input,
             gamepad_game_input,
             mouse_aim,
             dynamic_movement_keyboard,
             dynamic_movement_gamepad,
             kinematic_movement,
             // After movement so mouse facing wins over the A/D torque.
             mouse_face,
            ).chain().run_if(in_state(GameState::InGame)),
        );
    }
}
