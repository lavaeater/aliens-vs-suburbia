//! Gamepad control for players (twin-stick style).
//!
//! Left stick = movement in *world* space relative to the camera: pushing the stick up
//! walks the character up the screen, regardless of which way it is facing. This is
//! deliberately different from the keyboard's tank controls (rotate, then walk forward).
//!
//! Right stick = aiming. While it is deflected it drives `AutoAim` directly and the body
//! turns to face it; when it is released the character faces the way it walks, and the
//! closest-in-FOV `auto_aim` system takes over again while firing.
//!
//! R2 (`RightTrigger2`) fires — it sets `ControlCommand::Throw`, the same trigger the
//! keyboard's Space uses, so both throwing and equipped-weapon shooting pick it up.

use avian3d::prelude::AngularVelocity;
use bevy::app::{App, Plugin, PreUpdate};
use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;

use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::components::{CharacterControl, ControlCommand, InputKeyboard};
use crate::game_state::GameState;
use crate::player::components::{AutoAim, Player, PlayerDead};
use crate::settings::resources::GameSettings;

/// Sticks are considered idle below this deflection (on top of Bevy's own dead zone).
const STICK_DEAD_ZONE: f32 = 0.2;
/// R2 counts as pressed above this pull.
const TRIGGER_THRESHOLD: f32 = 0.3;

/// Marker component for entities controlled by a gamepad.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct InputGamepad {
    pub gamepad: Option<Entity>,
    /// True while the right stick is deflected, i.e. the player is aiming manually.
    /// `auto_aim` skips these players so the stick wins over target snapping.
    pub aim_active: bool,
}

impl InputGamepad {
    fn new(gamepad: Entity) -> Self {
        InputGamepad { gamepad: Some(gamepad), aim_active: false }
    }
}

pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<InputGamepad>()
            .register_type::<WantsGamepad>()
            .add_systems(
            PreUpdate,
            assign_gamepads.run_if(in_state(GameState::InGame)),
        );
        // `gamepad_game_input` itself is part of `StatefulControlPlugin`'s chain, so it is
        // ordered against the movement systems that consume what it writes.
    }
}

/// Placed on a player that joined on a gamepad in the setup screen, holding the index of
/// the pad it joined with. Resolved to the actual gamepad entity by `assign_gamepads` —
/// players spawn before the pad list is necessarily readable, hence the two steps.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct WantsGamepad(pub usize);

/// Bind gamepad entities to players. Runs every frame, so it picks up pads that were
/// already connected at spawn time as well as ones plugged in mid-game.
///
/// Players carrying `WantsGamepad` get the pad at that index. If nobody asked for one
/// (the roster is empty because the setup screen was skipped), a connected pad is handed
/// to the lone keyboard player instead, so plugging in a controller Just Works.
#[allow(clippy::type_complexity)]
fn assign_gamepads(
    gamepads: Query<Entity, With<Gamepad>>,
    claimed: Query<&InputGamepad>,
    wanting: Query<(Entity, &WantsGamepad), Without<InputGamepad>>,
    keyboard_players: Query<Entity, (With<Player>, With<InputKeyboard>, Without<WantsGamepad>)>,
    mut commands: Commands,
) {
    let pads: Vec<Entity> = gamepads.iter().collect();
    let is_free = |pad: Entity| !claimed.iter().any(|c| c.gamepad == Some(pad));

    for (player, wants) in wanting.iter() {
        if let Some(&pad) = pads.get(wants.0)
            && is_free(pad)
        {
            commands
                .entity(player)
                .remove::<InputKeyboard>()
                .remove::<WantsGamepad>()
                .insert(InputGamepad::new(pad));
        }
    }

    // Fallback: no roster, so nobody claimed a pad — let a connected one take over.
    if wanting.is_empty()
        && let Some(&pad) = pads.first()
        && is_free(pad)
        && let Some(player) = keyboard_players.iter().next()
    {
        commands
            .entity(player)
            .remove::<InputKeyboard>()
            .insert(InputGamepad::new(pad));
    }
}

/// Map a left/right stick deflection into world space, using the camera yaw so that
/// "stick up" is always "up the screen".
fn stick_to_world(stick: Vec2, camera_yaw_degrees: f32) -> Vec3 {
    Quat::from_rotation_y(camera_yaw_degrees.to_radians()) * Vec3::new(stick.x, 0.0, -stick.y)
}

#[allow(clippy::type_complexity)]
pub fn gamepad_game_input(
    gamepads: Query<&Gamepad>,
    settings: Res<GameSettings>,
    mut player_query: Query<
        (
            Entity,
            &mut CharacterControl,
            &mut InputGamepad,
            &mut AutoAim,
            &Transform,
            &mut AngularVelocity,
        ),
        (With<Player>, Without<PlayerDead>),
    >,
    mut anim_ew: MessageWriter<AnimationEvent>,
) {
    let yaw = settings.yaw_degrees;

    for (entity, mut controller, mut input_gamepad, mut aim, transform, mut angular) in
        player_query.iter_mut()
    {
        let Some(pad_entity) = input_gamepad.gamepad else { continue };
        let Ok(gamepad) = gamepads.get(pad_entity) else { continue };

        // ── Movement: world-space, camera relative ──────────────────────────
        let left = gamepad.left_stick();
        let moving = left.length() > STICK_DEAD_ZONE;
        let was_moving = controller.walk_direction.length_squared() > 0.01;

        let move_dir = if moving { stick_to_world(left, yaw) } else { Vec3::ZERO };
        controller.walk_direction = move_dir;
        // The body is steered toward the aim instead (below), so no tank torque.
        controller.torque = Vec3::ZERO;

        if moving && !was_moving {
            anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
        } else if !moving && was_moving {
            anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Walk));
        }

        // ── Aim: right stick if deflected, else follow the walk direction ───
        let right = gamepad.right_stick();
        input_gamepad.aim_active = right.length() > STICK_DEAD_ZONE;
        if input_gamepad.aim_active {
            aim.0 = stick_to_world(right, yaw).normalize();
        } else if moving {
            aim.0 = move_dir.normalize();
        }

        // Turn the body toward the aim by steering yaw angular velocity (self-damping:
        // zero once aligned), the same way `mouse_face` does for the keyboard player.
        if aim.0.length_squared() > 1e-4 {
            let forward = transform.rotation * Vec3::NEG_Z;
            // y of cross(forward, aim): sign is which way to turn, magnitude is sin(error).
            let cross_y = forward.z * aim.0.x - forward.x * aim.0.z;
            let max = controller.max_turn_speed.max(1.0);
            angular.0.y = (cross_y * 12.0).clamp(-max, max);
        }

        // ── Fire: R2 ────────────────────────────────────────────────────────
        let firing = gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0) > TRIGGER_THRESHOLD
            || gamepad.pressed(GamepadButton::RightTrigger2);
        if firing {
            controller.triggers.insert(ControlCommand::Throw);
        } else {
            controller.triggers.remove(&ControlCommand::Throw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stick_to_world;
    use bevy::prelude::*;

    #[test]
    fn stick_up_walks_away_from_the_camera() {
        // Default yaw 0: the camera sits on +Z looking toward -Z, so up the screen is -Z.
        let dir = stick_to_world(Vec2::new(0.0, 1.0), 0.0);
        assert!((dir - Vec3::NEG_Z).length() < 1e-5, "expected -Z, got {dir:?}");
    }

    #[test]
    fn stick_right_walks_right_on_screen() {
        let dir = stick_to_world(Vec2::new(1.0, 0.0), 0.0);
        assert!((dir - Vec3::X).length() < 1e-5, "expected +X, got {dir:?}");
    }

    #[test]
    fn yaw_rotates_the_movement_basis_with_the_camera() {
        // Camera swung 90 deg around: screen-up now points down -X.
        let dir = stick_to_world(Vec2::new(0.0, 1.0), 90.0);
        assert!((dir - Vec3::NEG_X).length() < 1e-4, "expected -X, got {dir:?}");
    }

    #[test]
    fn stick_magnitude_is_preserved_for_analog_speed() {
        let dir = stick_to_world(Vec2::new(0.0, 0.5), 0.0);
        assert!((dir.length() - 0.5).abs() < 1e-5, "half deflection stays half speed");
    }
}
