use bevy::prelude::{Query, With};
use bevy_xpbd_3d::components::{AngularVelocity, LinearVelocity};
use bevy_xpbd_3d::math::Vector3;
use bevy_xpbd_3d::prelude::Rotation;
use crate::control::components::{ControlDirection, Controller, ControlRotation, DynamicMovement};

pub fn dynamic_movement(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &Rotation, &Controller), With<DynamicMovement>>,
) {
    for (mut linear_velocity, mut angular_velocity, rotation, controller) in query.iter_mut() {
        let mut force = Vector3::ZERO;
        let mut torque = Vector3::ZERO;

        if controller.directions.contains(&ControlDirection::Forward) {
            force.z = -1.0;
        }
        if controller.directions.contains(&ControlDirection::Backward) {
            force.z = 1.0;
        }
        if controller.rotations.contains(&ControlRotation::Left) {
            torque.y = 1.0;
        }
        if controller.rotations.contains(&ControlRotation::Right) {
            torque.y = -1.0;
        }
        force = rotation.mul_vec3(force) * controller.speed;
        linear_velocity.x = force.x;
        linear_velocity.z = force.z;
        angular_velocity.0 = torque * controller.turn_speed;
    }
}