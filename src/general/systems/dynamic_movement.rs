use bevy::prelude::Query;
use bevy_xpbd_3d::math::Vector3;
use bevy_xpbd_3d::prelude::{ExternalForce, ExternalTorque, Rotation};
use crate::player::components::general::{Controller, Directions, Rotations};

pub fn dynamic_movement(
    mut query: Query<(&mut ExternalForce, &mut ExternalTorque, &Rotation, &Controller)>,
) {
    for (mut external_force, mut external_torque, rotation, controller) in query.iter_mut() {
        let mut force = Vector3::ZERO;
        let mut torque = Vector3::ZERO;

        if controller.directions.contains(&Directions::Forward) {
            force.z = 1.0;
        }
        if controller.directions.contains(&Directions::Backward) {
            force.z = -1.0;
        }
        if controller.rotations.contains(&Rotations::Left) {
            torque.y = -1.0;
        }
        if controller.rotations.contains(&Rotations::Right) {
            torque.y = 1.0;
        }
        external_force.apply_force(force);
        external_torque.apply_torque(torque);
    }
}