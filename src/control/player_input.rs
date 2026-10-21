//! Player input, one code path for every device.
//!
//! leafwing-input-manager reads the keys, mouse and pads; each player carries an
//! `InputMap<PlayerAction>` (kept in step with `ControlBindings` and the player's device
//! by `sync_input_maps`) and leafwing updates its `ActionState` in `PreUpdate`.
//! `player_input` turns that into what the rest of the game consumes: `CharacterControl`
//! triggers and walk direction, `AutoAim` for the right stick, and the build / weapon /
//! ability messages.
//!
//! Both schemes are twin-stick and camera relative: the move keys or the left stick walk
//! in *world* space rotated by `GameSettings::yaw_degrees` (up = up the screen), so A/D
//! strafe rather than rotate. The mouse aims through `mouse_aim`, the right stick here.
//! The body follows the direction of travel and the torso twists toward the aim, see
//! `player::systems::torso_twist`.

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::bindings::{ControlBindings, PlayerAction};
use crate::control::components::{CharacterControl, ControlCommand, GameInputBlock, InputKeyboard};
use crate::control::gamepad_input::{InputGamepad, stick_to_world};
use crate::game_state::PauseState;
use crate::player::components::{AutoAim, Player, PlayerDead};
use crate::player::events::building_events::{
    ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode,
};
use crate::player::systems::abilities::AbilityInput;
use crate::player::systems::loadout::{SwitchWeapon, WeaponSelect};
use crate::player::systems::shoot::ReloadRequest;
use crate::settings::resources::GameSettings;

/// Which map a player's `InputMap` was last built for, so `sync_input_maps` only
/// rebuilds when the device or the bindings change.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlScheme {
    KeyboardMouse,
    Gamepad(Entity),
}

/// Give every player the `InputMap` for its device, and rebuild them all when the
/// bindings change (the controls screen). A player still waiting for its pad
/// (`WantsGamepad`, no `InputGamepad` yet) gets none.
#[allow(clippy::type_complexity)]
pub fn sync_input_maps(
    bindings: Res<ControlBindings>,
    players: Query<
        (
            Entity,
            Has<InputKeyboard>,
            Option<&InputGamepad>,
            Option<&ControlScheme>,
        ),
        With<Player>,
    >,
    mut commands: Commands,
) {
    for (entity, keyboard, pad, current) in players.iter() {
        let wanted = match (keyboard, pad.and_then(|p| p.gamepad)) {
            (_, Some(pad)) => Some(ControlScheme::Gamepad(pad)),
            (true, None) => Some(ControlScheme::KeyboardMouse),
            (false, None) => None,
        };
        if wanted == current.copied() && !bindings.is_changed() {
            continue;
        }
        let mut player = commands.entity(entity);
        match wanted {
            Some(scheme @ ControlScheme::KeyboardMouse) => {
                player.insert((bindings.keyboard_map(), scheme));
            }
            Some(scheme @ ControlScheme::Gamepad(pad)) => {
                player.insert((bindings.gamepad_map(pad), scheme));
            }
            None => {
                player.remove::<(InputMap<PlayerAction>, ControlScheme)>();
            }
        }
    }
}

/// What a player's actions may do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputGate {
    Open,
    /// Everything but `Fire`: the pointer is over a tool panel, and clicking a button
    /// there must not shoot at it.
    NoFire,
    /// Nothing at all: the game is paused, or a text field or the inspector owns the keys.
    Closed,
}

/// Pure, so the rules can be tested without an app. Gamepads only stop for the pause:
/// they cannot click a panel or type into a field.
pub const fn input_gate(
    keyboard_player: bool,
    paused: bool,
    block: Option<GameInputBlock>,
) -> InputGate {
    if paused {
        return InputGate::Closed;
    }
    match block {
        Some(block) if keyboard_player && block.keyboard => InputGate::Closed,
        Some(block) if keyboard_player && block.mouse_aim => InputGate::NoFire,
        _ => InputGate::Open,
    }
}

/// Apply `input_gate` to each player's `ActionState`. A disabled state reads as all
/// released, so a key held when the gate shuts does not stay stuck down after it opens.
pub fn gate_player_input(
    pause: Option<Res<State<PauseState>>>,
    block: Option<Res<GameInputBlock>>,
    mut players: Query<(&mut ActionState<PlayerAction>, Has<InputKeyboard>), With<Player>>,
) {
    let paused = pause.is_some_and(|p| *p.get() == PauseState::Paused);
    for (mut actions, keyboard) in players.iter_mut() {
        let gate = input_gate(keyboard, paused, block.as_deref().copied());
        match gate {
            InputGate::Closed => {
                if !actions.disabled() {
                    actions.disable();
                }
            }
            InputGate::Open | InputGate::NoFire => {
                if actions.disabled() {
                    actions.enable();
                }
                let fire_blocked = gate == InputGate::NoFire;
                if fire_blocked != actions.action_disabled(&PlayerAction::Fire) {
                    if fire_blocked {
                        actions.disable_action(&PlayerAction::Fire);
                    } else {
                        actions.enable_action(&PlayerAction::Fire);
                    }
                }
            }
        }
    }
}

/// The four move buttons as a stick: right/up positive, like `GamepadStick`.
fn move_buttons(actions: &ActionState<PlayerAction>) -> Vec2 {
    let axis = |pos: PlayerAction, neg: PlayerAction| {
        f32::from(u8::from(actions.pressed(&pos))) - f32::from(u8::from(actions.pressed(&neg)))
    };
    Vec2::new(
        axis(PlayerAction::MoveRight, PlayerAction::MoveLeft),
        axis(PlayerAction::MoveUp, PlayerAction::MoveDown),
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn player_input(
    settings: Res<GameSettings>,
    bindings: Res<ControlBindings>,
    mut players: Query<
        (
            Entity,
            &ActionState<PlayerAction>,
            &mut CharacterControl,
            &mut AutoAim,
            Option<&mut InputGamepad>,
        ),
        (With<Player>, Without<PlayerDead>),
    >,
    mut anim_ew: MessageWriter<AnimationEvent>,
    mut enter_build_ew: MessageWriter<EnterBuildMode>,
    mut exit_build_ew: MessageWriter<ExitBuildMode>,
    mut execute_build_ew: MessageWriter<ExecuteBuild>,
    mut change_build_indicator_ew: MessageWriter<ChangeBuildIndicator>,
    mut ability_input: Option<ResMut<AbilityInput>>,
    mut reload_mw: MessageWriter<ReloadRequest>,
    mut switch_mw: MessageWriter<SwitchWeapon>,
) {
    let yaw = settings.yaw_degrees;

    for (entity, actions, mut controller, mut aim, pad) in players.iter_mut() {
        // ── Movement: world space, camera relative ──────────────────────────
        // Keys and stick are summed so a pad player can bind the d-pad to the move
        // buttons too; the clamp keeps diagonals at full speed, not 1.41x.
        let walk =
            (move_buttons(actions) + actions.axis_pair(&PlayerAction::Move)).clamp_length_max(1.0);
        let moving = walk.length_squared() > 1e-4;
        let was_moving = controller.walk_direction.length_squared() > 1e-4;
        let move_dir = if moving {
            stick_to_world(walk, yaw)
        } else {
            Vec3::ZERO
        };
        controller.walk_direction = move_dir;
        // The body is steered by `face_movement_direction`, so no torque.
        controller.torque = Vec3::ZERO;

        if moving && !was_moving {
            anim_ew.write(AnimationEvent(
                AnimationEventType::GotoAnimState,
                entity,
                AnimationKey::Walk,
            ));
        } else if !moving && was_moving {
            anim_ew.write(AnimationEvent(
                AnimationEventType::LeaveAnimState,
                entity,
                AnimationKey::Walk,
            ));
        }

        // ── Aim: right stick if deflected, else follow the walk direction ───
        // Keyboard players aim with the mouse instead (`mouse_aim`, later in the chain).
        if let Some(mut pad) = pad {
            let right = actions.axis_pair(&PlayerAction::Aim);
            pad.aim_active = right.length_squared() > 1e-4;
            if pad.aim_active {
                aim.0 = stick_to_world(right, yaw).normalize();
            } else if moving {
                aim.0 = move_dir.normalize();
            }
        }

        // ── Build mode ──────────────────────────────────────────────────────
        let in_build_mode = controller.triggers.contains(&ControlCommand::Build);
        let leave_build =
            |anim_ew: &mut MessageWriter<AnimationEvent>,
             exit_build_ew: &mut MessageWriter<ExitBuildMode>| {
                anim_ew.write(AnimationEvent(
                    AnimationEventType::LeaveAnimState,
                    entity,
                    AnimationKey::Building,
                ));
                exit_build_ew.write(ExitBuildMode(entity));
            };

        if actions.just_pressed(&PlayerAction::BuildMode) {
            if in_build_mode {
                leave_build(&mut anim_ew, &mut exit_build_ew);
            } else {
                controller.triggers.insert(ControlCommand::Build);
                anim_ew.write(AnimationEvent(
                    AnimationEventType::GotoAnimState,
                    entity,
                    AnimationKey::Building,
                ));
                enter_build_ew.write(EnterBuildMode(entity));
            }
        } else if in_build_mode {
            if actions.just_pressed(&PlayerAction::CancelBuild) {
                leave_build(&mut anim_ew, &mut exit_build_ew);
            } else if actions.just_pressed(&PlayerAction::PlaceBuild) {
                execute_build_ew.write(ExecuteBuild(entity));
            }
            if actions.just_pressed(&PlayerAction::NextBuildItem) {
                change_build_indicator_ew.write(ChangeBuildIndicator(entity, 1));
            }
            if actions.just_pressed(&PlayerAction::PrevBuildItem) {
                change_build_indicator_ew.write(ChangeBuildIndicator(entity, -1));
            }
        }

        // ── Interact (revive), held ─────────────────────────────────────────
        if actions.pressed(&PlayerAction::Interact) {
            controller.triggers.insert(ControlCommand::Interact);
        } else {
            controller.triggers.remove(&ControlCommand::Interact);
        }

        if actions.just_pressed(&PlayerAction::ThrowSpecial) {
            controller.triggers.insert(ControlCommand::ThrowSpecial);
        }

        // ── Weapons ─────────────────────────────────────────────────────────
        if actions.just_pressed(&PlayerAction::Reload) {
            reload_mw.write(ReloadRequest(entity));
        }
        let selects = [
            (PlayerAction::NextWeapon, WeaponSelect::Next),
            (PlayerAction::PrevWeapon, WeaponSelect::Prev),
            (PlayerAction::Weapon1, WeaponSelect::Slot(0)),
            (PlayerAction::Weapon2, WeaponSelect::Slot(1)),
            (PlayerAction::Weapon3, WeaponSelect::Slot(2)),
            (PlayerAction::Weapon4, WeaponSelect::Slot(3)),
        ];
        for (action, select) in selects {
            if actions.just_pressed(&action) {
                switch_mw.write(SwitchWeapon {
                    player: entity,
                    select,
                });
            }
        }

        // ── Ability ─────────────────────────────────────────────────────────
        if actions.just_pressed(&PlayerAction::Ability)
            && let Some(ref mut ai) = ability_input
        {
            ai.pressed = true;
        }

        // ── Fire, held ──────────────────────────────────────────────────────
        // Analog: a trigger counts past `trigger_threshold`, earlier than Bevy's own
        // press threshold; a key or mouse button reads 1.0. Suppressed while building,
        // where the same buttons place a tile instead.
        let firing = !in_build_mode
            && actions.button_value(&PlayerAction::Fire) > bindings.trigger_threshold;
        if firing {
            controller.triggers.insert(ControlCommand::Throw);
        } else {
            controller.triggers.remove(&ControlCommand::Throw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_a_normal_match_everything_gets_through() {
        assert_eq!(input_gate(true, false, None), InputGate::Open);
        assert_eq!(input_gate(false, false, None), InputGate::Open);
    }

    #[test]
    fn the_pause_stops_every_device() {
        assert_eq!(input_gate(true, true, None), InputGate::Closed);
        assert_eq!(input_gate(false, true, None), InputGate::Closed);
    }

    /// The point of `NoFire`: clicking an editor button must not shoot at it, but the
    /// keys still walk.
    #[test]
    fn over_a_panel_the_keyboard_player_walks_but_does_not_fire() {
        let block = GameInputBlock {
            keyboard: false,
            mouse_aim: true,
        };
        assert_eq!(input_gate(true, false, Some(block)), InputGate::NoFire);
    }

    #[test]
    fn a_text_field_takes_the_keys_but_not_the_pad() {
        let block = GameInputBlock {
            keyboard: true,
            mouse_aim: true,
        };
        assert_eq!(input_gate(true, false, Some(block)), InputGate::Closed);
        assert_eq!(input_gate(false, false, Some(block)), InputGate::Open);
    }

    #[test]
    fn opposite_move_buttons_cancel_out() {
        let mut actions = ActionState::<PlayerAction>::default();
        actions.press(&PlayerAction::MoveLeft);
        actions.press(&PlayerAction::MoveRight);
        actions.press(&PlayerAction::MoveUp);
        assert_eq!(move_buttons(&actions), Vec2::new(0.0, 1.0));
    }
}
