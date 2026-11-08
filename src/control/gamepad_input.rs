//! Gamepad players: which pad drives which player.
//!
//! Reading the pad is leafwing's job (`control::player_input`); a pad player carries an
//! `InputMap` tied to its pad, built by `ControlBindings::gamepad_map`. This module binds
//! pads to players (`assign_gamepads`) and holds the stick-to-world maths both schemes
//! share.

use std::f32::consts::{PI, TAU};

use bevy::app::{App, Plugin, PreUpdate};
use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;

use crate::control::components::InputKeyboard;
use crate::game_state::GameState;
use crate::player::components::Player;

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
    const fn new(gamepad: Entity) -> Self {
        Self {
            gamepad: Some(gamepad),
            aim_active: false,
        }
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
pub fn stick_to_world(stick: Vec2, camera_yaw_degrees: f32) -> Vec3 {
    Quat::from_rotation_y(camera_yaw_degrees.to_radians()) * Vec3::new(stick.x, 0.0, -stick.y)
}

/// Turns the aim `current` toward `target` (both on the ground plane) by the shorter way,
/// covering `1 - e^(-rate * dt)` of the angle: an exponential ease that is frame-rate
/// independent and soaks up a jittery stick. `rate <= 0` (or no aim yet) snaps.
pub fn steer_aim(current: Vec3, target: Vec3, rate: f32, dt: f32) -> Vec3 {
    if rate <= 0.0 || current.xz().length_squared() < 1e-6 {
        return target;
    }
    let yaw = |v: Vec3| v.x.atan2(v.z);
    let from = yaw(current);
    let delta = (yaw(target) - from + PI).rem_euclid(TAU) - PI;
    let turned = from + delta * (1.0 - (-rate * dt).exp());
    Vec3::new(turned.sin(), 0.0, turned.cos())
}

#[cfg(test)]
mod tests {
    use super::{steer_aim, stick_to_world};
    use bevy::prelude::*;

    #[test]
    fn the_aim_eases_toward_the_stick_the_short_way_round() {
        let east = Vec3::X;
        // 170 degrees clockwise from +Z vs 170 counter-clockwise: 20 degrees apart via -Z.
        let a = Vec3::new(10f32.to_radians().sin(), 0.0, -10f32.to_radians().cos());
        let b = Vec3::new(-10f32.to_radians().sin(), 0.0, -10f32.to_radians().cos());
        let step = steer_aim(a, b, 10.0, 1.0 / 60.0);
        assert!(step.x < a.x && step.x > b.x, "turned through -Z, not +Z: {step}");
        assert!(step.z < -0.9);
        // Unit length, on the ground, and it gets there.
        assert!((step.length() - 1.0).abs() < 1e-5 && step.y == 0.0);
        let mut aim = east;
        for _ in 0..60 {
            aim = steer_aim(aim, Vec3::Z, 18.0, 1.0 / 60.0);
        }
        assert!(aim.distance(Vec3::Z) < 1e-3, "settled in a second: {aim}");
    }

    #[test]
    fn no_smoothing_or_no_previous_aim_snaps() {
        assert_eq!(steer_aim(Vec3::X, Vec3::Z, 0.0, 0.016), Vec3::Z);
        assert_eq!(steer_aim(Vec3::ZERO, Vec3::Z, 18.0, 0.016), Vec3::Z);
    }

    #[test]
    fn stick_up_walks_away_from_the_camera() {
        // Default yaw 0: the camera sits on +Z looking toward -Z, so up the screen is -Z.
        let dir = stick_to_world(Vec2::new(0.0, 1.0), 0.0);
        assert!(
            (dir - Vec3::NEG_Z).length() < 1e-5,
            "expected -Z, got {dir:?}"
        );
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
        assert!(
            (dir - Vec3::NEG_X).length() < 1e-4,
            "expected -X, got {dir:?}"
        );
    }

    #[test]
    fn stick_magnitude_is_preserved_for_analog_speed() {
        let dir = stick_to_world(Vec2::new(0.0, 0.5), 0.0);
        assert!(
            (dir.length() - 0.5).abs() < 1e-5,
            "half deflection stays half speed"
        );
    }
}
