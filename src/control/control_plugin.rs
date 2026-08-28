use bevy::app::{App, Plugin, PreUpdate, Update};
use bevy::prelude::{in_state, IntoScheduleConfigs};
use crate::general::systems::dynamic_movement_system::dynamic_movement;
use crate::general::systems::kinematic_movement_system::kinematic_movement;
use crate::control::gamepad_input::gamepad_game_input;
use crate::control::keyboard_input::{keyboard_input};
use crate::control::mouse_aim::mouse_aim;
use crate::game_state::GameState;
use crate::player::systems::torso_twist::face_movement_direction;

#[allow(dead_code)]
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
            PreUpdate,
            (keyboard_input,
             gamepad_game_input,
             mouse_aim,
             dynamic_movement,
             kinematic_movement,
             // After movement so the body facing wins over any leftover torque. The
             // body follows the direction of travel; `apply_torso_twist` (PostUpdate)
             // then bends the upper body the rest of the way to the aim.
             face_movement_direction,
            ).chain().run_if(in_state(GameState::InGame)),
        );
    }
}
