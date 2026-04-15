use bevy::math::{EulerRot, Quat};
use bevy::prelude::{Query, Transform, With};
use avian3d::prelude::{AngularVelocity, LinearVelocity};
use crate::control::components::{CharacterControl, DynamicMovement, InputKeyboard};
use crate::control::gamepad_input::InputGamepad;

pub fn dynamic_movement_keyboard(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &mut Transform, &CharacterControl), (With<DynamicMovement>, With<InputKeyboard>)>,
) {
    for (mut linear_velocity, mut angular_velocity, mut transform, controller) in query.iter_mut() {
        let force = transform.rotation.mul_vec3(controller.walk_direction) * controller.speed;
        linear_velocity.x = force.x;
        linear_velocity.z = force.z;
        angular_velocity.0 = controller.torque * controller.turn_speed;
    }
}


pub fn dynamic_movement_gamepad(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &mut Transform, &CharacterControl), (With<DynamicMovement>, With<InputGamepad>)>,
) {
    for (mut linear_velocity, mut angular_velocity, mut transform, controller) in query.iter_mut() {
        linear_velocity.x = 0.0;
        linear_velocity.z = 0.0;
        let direction = Quat::from_euler(EulerRot::YXZ, 45.0f32.to_radians(), 0.0, 0.0).mul_vec3(controller.walk_direction);
        linear_velocity.x = direction.x * controller.speed;
        linear_velocity.z = direction.z * controller.speed;
    }
}
