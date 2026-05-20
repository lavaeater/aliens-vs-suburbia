use bevy::prelude::{info, Query, Vec3, With};
use avian3d::prelude::{AngularVelocity, LinearVelocity, Rotation};
use crate::control::components::{ControlDirection, CharacterControl, ControlRotation, KinematicMovement};

pub fn kinematic_movement(
    mut query: Query<(&mut LinearVelocity, &mut AngularVelocity, &Rotation, &CharacterControl), With<KinematicMovement>>,
) {
    let force_factor = 1.0;
    for (
        mut linear_velocity,
        mut angular_velocity,
        rotation,
        controller) in query.iter_mut() {
        let mut force = Vec3::ZERO;
        let mut torque = Vec3::ZERO;

        if controller.directions.contains(&ControlDirection::Forward) {
            info!("Move forward!");
            force.z = -1.0;
        }
        if controller.directions.contains(&ControlDirection::Backward) {
            info!("Move backwards!");
            force.z = 1.0;
        }
        if controller.rotations.contains(&ControlRotation::Left) {
            info!("Rotate left!");
            torque.y = -1.0;
        }
        if controller.rotations.contains(&ControlRotation::Right) {
            info!("Rotate right!");
            torque.y = 1.0;
        }
        force = rotation.0.mul_vec3(force);
        linear_velocity.0 = force * force_factor;
        angular_velocity.0 = torque * force_factor;
    }
}
