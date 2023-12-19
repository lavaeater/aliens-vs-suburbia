use bevy::input::keyboard::KeyboardInput;
use bevy::math::{EulerRot, Quat, Vec2, Vec3Swizzles};
use bevy::prelude::{Has, Query, Transform, With};
use bevy_xpbd_3d::components::{AngularVelocity, LinearVelocity};
use bevy_xpbd_3d::math::Vector3;
use bevy_xpbd_3d::prelude::Rotation;
use crate::control::components::{ControlDirection, CharacterControl, ControlRotation, DynamicMovement, InputKeyboard, CharacterControlType};

pub fn dynamic_movement(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &mut Transform, &CharacterControl), With<DynamicMovement>>,
) {
    for (mut linear_velocity, mut angular_velocity, mut transform, controller) in query.iter_mut() {
        match controller.control_type {
            CharacterControlType::Keyboard => {
                let force = transform.rotation.mul_vec3(controller.walk_direction.clone()) * controller.speed;
                linear_velocity.x = force.x;
                linear_velocity.z = force.z;
                angular_velocity.0 = controller.torque * controller.turn_speed;
            }
            CharacterControlType::Gamepad => {
                linear_velocity.x = controller.walk_direction.x * controller.speed;
                linear_velocity.z = controller.walk_direction.z * controller.speed;

                transform.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    controller.walk_direction.xz().angle_between(Vec2::Z),
                    0.0,
                    0.0
                );
            }
        }
    }
}