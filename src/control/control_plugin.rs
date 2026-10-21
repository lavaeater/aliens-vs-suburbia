use crate::control::bindings::{ControlBindings, PlayerAction};
use crate::control::mouse_aim::mouse_aim;
use crate::control::player_input::{gate_player_input, player_input, sync_input_maps};
use crate::game_state::GameState;
use crate::general::systems::dynamic_movement_system::dynamic_movement;
use crate::general::systems::kinematic_movement_system::kinematic_movement;
use crate::player::systems::torso_twist::face_movement_direction;
use bevy::app::{App, Plugin, PreUpdate};
use bevy::prelude::{IntoScheduleConfigs, in_state};
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::InputManagerPlugin;

pub struct StatefulControlPlugin;

impl Plugin for StatefulControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerAction>::default())
            .insert_resource(ControlBindings::load_or_write_default())
            .add_systems(
                PreUpdate,
                (
                    sync_input_maps,
                    // After leafwing has read the devices, so a gate that shuts this frame
                    // already hides this frame's presses.
                    gate_player_input,
                    player_input,
                    mouse_aim,
                    dynamic_movement,
                    kinematic_movement,
                    // After movement so the body facing wins over any leftover torque. The
                    // body follows the direction of travel; `apply_torso_twist` (PostUpdate)
                    // then bends the upper body the rest of the way to the aim.
                    face_movement_direction,
                )
                    .chain()
                    .after(InputManagerSystem::ManualControl)
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
