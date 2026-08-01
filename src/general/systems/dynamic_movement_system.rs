#![allow(clippy::type_complexity)]
use bevy::prelude::{Query, Transform, With};
use avian3d::prelude::{AngularVelocity, LinearVelocity};
use crate::control::components::{CharacterControl, DynamicMovement, InputKeyboard};
use crate::control::gamepad_input::InputGamepad;

pub fn dynamic_movement_keyboard(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &mut Transform, &CharacterControl), (With<DynamicMovement>, With<InputKeyboard>)>,
) {
    for (mut linear_velocity, mut angular_velocity, transform, controller) in query.iter_mut() {
        let force = transform.rotation.mul_vec3(controller.walk_direction) * controller.speed;
        linear_velocity.x = force.x;
        linear_velocity.z = force.z;
        angular_velocity.0 = controller.torque * controller.turn_speed;
    }
}


/// Gamepad players move in world space: `walk_direction` is already camera-relative
/// (see `control::gamepad_input`), so it is applied as-is rather than rotated by the
/// body's facing. That way the character can strafe while aiming somewhere else.
pub fn dynamic_movement_gamepad(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &mut Transform, &CharacterControl), (With<DynamicMovement>, With<InputGamepad>)>,
) {
    for (mut linear_velocity, _, _, controller) in query.iter_mut() {
        linear_velocity.x = controller.walk_direction.x * controller.speed;
        linear_velocity.z = controller.walk_direction.z * controller.speed;
    }
}
