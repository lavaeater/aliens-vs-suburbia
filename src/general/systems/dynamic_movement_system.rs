#![allow(clippy::type_complexity)]
use bevy::prelude::{Query, With};
use avian3d::prelude::LinearVelocity;
use crate::control::components::{CharacterControl, DynamicMovement};

/// Move in world space. Both input paths now write a camera-relative `walk_direction`
/// (keyboard WASD via `keyboard_input`, gamepad left stick via `gamepad_game_input`),
/// so it is applied as-is rather than rotated by the body's facing. That decoupling is
/// what lets the character strafe while the torso aims somewhere else — the body is
/// steered separately by `face_movement_direction`.
pub fn dynamic_movement(
    mut query: Query<(&mut LinearVelocity, &CharacterControl), With<DynamicMovement>>,
) {
    for (mut linear_velocity, controller) in query.iter_mut() {
        linear_velocity.x = controller.walk_direction.x * controller.speed;
        linear_velocity.z = controller.walk_direction.z * controller.speed;
    }
}
