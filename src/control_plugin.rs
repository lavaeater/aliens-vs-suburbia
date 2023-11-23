use bevy::app::{App, Plugin, Update};
use crate::general::systems::dynamic_movement_system::dynamic_movement;
use crate::general::systems::kinematic_movement_system::kinematic_movement;
use crate::player::systems::keyboard_control::input_control;

pub struct ControlPlugin;

impl Plugin for ControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (input_control,
             dynamic_movement,
             kinematic_movement,
            )
        );
    }
}
