#![allow(clippy::type_complexity)]
use bevy::prelude::{Query, Res, With};
use avian3d::prelude::LinearVelocity;
use crate::control::components::{CharacterControl, DynamicMovement};
use crate::settings::resources::GameSettings;

/// Move in world space. Both input paths now write a camera-relative `walk_direction`
/// (keyboard WASD via `keyboard_input`, gamepad left stick via `gamepad_game_input`),
/// so it is applied as-is rather than rotated by the body's facing. That decoupling is
/// what lets the character strafe while the torso aims somewhere else — the body is
/// steered separately by `face_movement_direction`.
///
/// `GameSettings::player_speed_multiplier` scales the result. Only the player has
/// `DynamicMovement` — aliens are moved kinematically — so this is the player's speed and
/// nothing else's. The setting and its slider have existed for a while; until now nothing
/// read them, so the walk ran at a fixed 3 m/s no matter what the panel said.
pub fn dynamic_movement(
    settings: Res<GameSettings>,
    mut query: Query<(&mut LinearVelocity, &CharacterControl), With<DynamicMovement>>,
) {
    for (mut linear_velocity, controller) in query.iter_mut() {
        let speed = controller.speed * settings.player_speed_multiplier;
        linear_velocity.x = controller.walk_direction.x * speed;
        linear_velocity.z = controller.walk_direction.z * speed;
    }
}
