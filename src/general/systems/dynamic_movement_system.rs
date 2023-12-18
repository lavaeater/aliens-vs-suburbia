use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{Has, Query, With};
use bevy_xpbd_3d::components::{AngularVelocity, LinearVelocity};
use bevy_xpbd_3d::math::Vector3;
use bevy_xpbd_3d::prelude::Rotation;
use crate::control::components::{ControlDirection, CharacterControl, ControlRotation, DynamicMovement, InputKeyboard};

pub fn dynamic_movement(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &mut Rotation, &CharacterControl, Has<InputKeyboard>), With<DynamicMovement>>,
) {
    for (mut linear_velocity, mut angular_velocity, mut rotation, controller, keyboard) in query.iter_mut() {
        if keyboard {
            let force = rotation.mul_vec3(controller.force.clone()) * controller.speed;
            linear_velocity.x = force.x;
            linear_velocity.z = force.z;
            angular_velocity.0 = controller.torque * controller.turn_speed;
        } else {
            linear_velocity.x = controller.force.x * controller.speed;
            linear_velocity.z = controller.force.z * controller.speed;
            rotation = 
        }
        //  fuuuuck

    }
}