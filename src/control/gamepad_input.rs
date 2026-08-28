//! Gamepad control for players (twin-stick style).
//!
//! Left stick = movement in *world* space relative to the camera: pushing the stick up
//! walks the character up the screen, regardless of which way it is facing. This is
//! the same scheme the keyboard's WASD now uses.
//!
//! Right stick = aiming. While it is deflected it drives `AutoAim` directly; when it is
//! released the closest-in-FOV `auto_aim` system takes over again while firing. The body
//! itself follows the direction of travel and the torso twists toward the aim -- see
//! `player::systems::torso_twist`.
//!
//! Buttons (fire, build, ability, ...) are remappable — see `control::bindings` and
//! `gamepad-bindings.ron`. Firing sets `ControlCommand::Throw`, the same trigger the
//! keyboard's Space uses, so both throwing and equipped-weapon shooting pick it up; the
//! build buttons write the same `EnterBuildMode`/`ExecuteBuild`/... messages as B/Space/
//! Escape/arrows do.

use bevy::app::{App, Plugin, PreUpdate};
use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;

use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::bindings::GamepadBindings;
use crate::control::components::{CharacterControl, ControlCommand, InputKeyboard};
use crate::game_state::GameState;
use crate::player::components::{AutoAim, Player, PlayerDead};
use crate::player::events::building_events::{
    ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode,
};
use crate::player::systems::abilities::AbilityInput;
use crate::settings::resources::GameSettings;

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
        app.insert_resource(GamepadBindings::load_or_write_default())
            .register_type::<InputGamepad>()
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
pub fn stick_to_world(stick: Vec2, camera_yaw_degrees: f32) -> Vec3 {
    Quat::from_rotation_y(camera_yaw_degrees.to_radians()) * Vec3::new(stick.x, 0.0, -stick.y)
}

/// Is an analog button pulled far enough to count as held? Triggers report a value in
/// [0, 1]; anything digital falls back to its pressed state.
fn is_held(gamepad: &Gamepad, button: GamepadButton, threshold: f32) -> bool {
    match gamepad.get(button) {
        Some(value) => value > threshold,
        None => gamepad.pressed(button),
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn gamepad_game_input(
    gamepads: Query<&Gamepad>,
    settings: Res<GameSettings>,
    bindings: Res<GamepadBindings>,
    mut player_query: Query<
        (
            Entity,
            &mut CharacterControl,
            &mut InputGamepad,
            &mut AutoAim,
        ),
        (With<Player>, Without<PlayerDead>),
    >,
    mut anim_ew: MessageWriter<AnimationEvent>,
    mut enter_build_ew: MessageWriter<EnterBuildMode>,
    mut exit_build_ew: MessageWriter<ExitBuildMode>,
    mut execute_build_ew: MessageWriter<ExecuteBuild>,
    mut change_build_indicator_ew: MessageWriter<ChangeBuildIndicator>,
    mut ability_input: Option<ResMut<AbilityInput>>,
) {
    let yaw = settings.yaw_degrees;
    let dead_zone = bindings.stick_dead_zone;

    for (entity, mut controller, mut input_gamepad, mut aim) in player_query.iter_mut()
    {
        let Some(pad_entity) = input_gamepad.gamepad else { continue };
        let Ok(gamepad) = gamepads.get(pad_entity) else { continue };

        // ── Movement: world-space, camera relative ──────────────────────────
        let left = gamepad.left_stick();
        let moving = left.length() > dead_zone;
        let was_moving = controller.walk_direction.length_squared() > 0.01;

        let move_dir = if moving { stick_to_world(left, yaw) } else { Vec3::ZERO };
        controller.walk_direction = move_dir;
        // The body is steered separately (see face_movement_direction), so no torque.
        controller.torque = Vec3::ZERO;

        if moving && !was_moving {
            anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
        } else if !moving && was_moving {
            anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Walk));
        }

        // ── Aim: right stick if deflected, else follow the walk direction ───
        let right = gamepad.right_stick();
        input_gamepad.aim_active = right.length() > dead_zone;
        if input_gamepad.aim_active {
            aim.0 = stick_to_world(right, yaw).normalize();
        } else if moving {
            aim.0 = move_dir.normalize();
        }

        // Body facing is handled uniformly for every player by
        // `torso_twist::face_movement_direction`, which steers the hips toward the
        // direction of travel and lets the spine cover the rest of the way to the aim.

        // ── Build mode ──────────────────────────────────────────────────────
        // The one-shot actions use the digital edge, so a button bound to a trigger
        // still fires once per pull rather than every frame it is held.
        let in_build_mode = controller.triggers.contains(&ControlCommand::Build);

        if gamepad.just_pressed(bindings.build_mode) {
            if in_build_mode {
                anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                exit_build_ew.write(ExitBuildMode(entity));
            } else {
                controller.triggers.insert(ControlCommand::Build);
                anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Building));
                enter_build_ew.write(EnterBuildMode(entity));
            }
        } else if in_build_mode {
            if gamepad.just_pressed(bindings.exit_build) {
                anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                exit_build_ew.write(ExitBuildMode(entity));
            } else if gamepad.just_pressed(bindings.execute_build) {
                execute_build_ew.write(ExecuteBuild(entity));
            }

            if gamepad.just_pressed(bindings.next_build_item) {
                change_build_indicator_ew.write(ChangeBuildIndicator(entity, 1));
            }
            if gamepad.just_pressed(bindings.prev_build_item) {
                change_build_indicator_ew.write(ChangeBuildIndicator(entity, -1));
            }
        }

        // ── Ability ─────────────────────────────────────────────────────────
        if gamepad.just_pressed(bindings.ability)
            && let Some(ref mut ai) = ability_input
        {
            ai.pressed = true;
        }

        // ── Fire ────────────────────────────────────────────────────────────
        // Suppressed while building, matching the keyboard, where Space places a tile
        // instead of throwing.
        let firing = !in_build_mode && is_held(gamepad, bindings.fire, bindings.trigger_threshold);
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
