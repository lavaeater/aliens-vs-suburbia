use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::math::Vec3;
use bevy::prelude::{Entity, EventReader, EventWriter, KeyCode, Query, With};
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::components::{CharacterControl, ControllerFlag, InputKeyboard};
use crate::player::events::building_events::{ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode};

pub fn keyboard_input(
    mut key_evr: EventReader<KeyboardInput>,
    mut query: Query<(Entity, &mut CharacterControl), With<InputKeyboard>>,
    mut start_build_ew: EventWriter<EnterBuildMode>,
    mut execute_build: EventWriter<ExecuteBuild>,
    mut exit_build: EventWriter<ExitBuildMode>,
    mut change_build_indicator: EventWriter<ChangeBuildIndicator>,
    mut animation_ew: EventWriter<AnimationEvent>,
) {
    if let Ok((entity, mut controller)) = query.get_single_mut() {
        for ev in key_evr.read() {
            match ev.state {
                ButtonState::Pressed => match ev.key_code {
                    KeyCode::KeyB => {
                        if controller.triggers.has(ControllerFlag::BUILD) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        } else {
                            controller.triggers.set(ControllerFlag::BUILD);
                            animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Building));
                            start_build_ew.send(EnterBuildMode(entity));
                        }
                    }
                    KeyCode::Escape => {
                        if controller.triggers.has(ControllerFlag::BUILD) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        }
                    }
                    KeyCode::KeyA => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.rotations.set(ControllerFlag::LEFT);
                    }
                    KeyCode::KeyD => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.rotations.set(ControllerFlag::RIGHT);
                    }
                    KeyCode::KeyW => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.set(ControllerFlag::FORWARD);
                    }
                    KeyCode::KeyS => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.set(ControllerFlag::BACKWARD);
                    }
                    KeyCode::KeyN => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.directions.set(ControllerFlag::BACKWARD);
                    }
                    KeyCode::Space => {
                        if controller.triggers.has(ControllerFlag::BUILD) {
                            execute_build.send(ExecuteBuild(entity));
                        } else if controller.triggers.has(ControllerFlag::THROW) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Throwing));
                            controller.triggers.unset(ControllerFlag::THROW);
                        } else {
                            animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Throwing));
                            controller.triggers.set(ControllerFlag::THROW);
                        }
                    }
                    all_other => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, key_for_input(all_other)));
                    }
                },
                ButtonState::Released => match ev.key_code {
                    KeyCode::KeyA => {
                        controller.rotations.unset(ControllerFlag::LEFT);
                    }
                    KeyCode::KeyD => {
                        controller.rotations.unset(ControllerFlag::RIGHT);
                    }
                    KeyCode::KeyW => {
                        controller.directions.unset(ControllerFlag::FORWARD);
                    }
                    KeyCode::KeyS => {
                        controller.directions.unset(ControllerFlag::BACKWARD);
                    }
                    KeyCode::ArrowLeft => {
                        change_build_indicator.send(ChangeBuildIndicator(entity, -1));
                    }
                    KeyCode::ArrowRight => {
                        change_build_indicator.send(ChangeBuildIndicator(entity, 1));
                    }
                    _ => {}
                }
            }
            if controller.directions.is_empty() && controller.rotations.is_empty() {
                animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Walking));
            }

            controller.walk_direction = Vec3::ZERO;
            controller.torque = Vec3::ZERO;

            if controller.directions.has(ControllerFlag::FORWARD) {
                controller.walk_direction.z = -1.0;
            }
            if controller.directions.has(ControllerFlag::BACKWARD) {
                controller.walk_direction.z = 1.0;
            }
            if controller.rotations.has(ControllerFlag::LEFT) {
                controller.torque.y = 1.0;
            }
            if controller.rotations.has(ControllerFlag::RIGHT) {
                controller.torque.y = -1.0;
            }
        }
    }
}

fn key_for_input(key_code: KeyCode) -> AnimationKey {
    match key_code {
        KeyCode::Digit1 => AnimationKey::Idle, //OK
        KeyCode::Digit2 => AnimationKey::Walk, //OK
        KeyCode::Digit3 => AnimationKey::Yes, //OK
        KeyCode::Digit4 => AnimationKey::Wave, //OK
        KeyCode::Digit5 => AnimationKey::RunGun,
        KeyCode::Digit6 => AnimationKey::Run,
        KeyCode::Digit7 => AnimationKey::Punch,
        KeyCode::Digit8 => AnimationKey::No, //OK
        KeyCode::Digit9 => AnimationKey::JumpLand, //Punch?
        KeyCode::Digit0 => AnimationKey::JumpIdle, //JumpMidAir, JumpIdle, OK?
        KeyCode::Numpad0 => AnimationKey::Jump, //Run
        KeyCode::Numpad1 => AnimationKey::IdleShoot, //OK
        KeyCode::Numpad2 => AnimationKey::HitReact, //OK
        KeyCode::Numpad3 => AnimationKey::Duck, //OK
        KeyCode::Numpad4 => AnimationKey::Death, //OK
        KeyCode::Numpad5 => AnimationKey::WalkShoot, //OK
        KeyCode::Numpad6 => AnimationKey::RunShoot, //OK
        _ => AnimationKey::Idle
    }
}