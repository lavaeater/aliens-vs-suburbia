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
                    Some(KeyCode::B) => {
                        if controller.triggers.has(ControllerFlag::BUILD) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        } else {
                            controller.triggers.set(ControllerFlag::BUILD);
                            animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Building));
                            start_build_ew.send(EnterBuildMode(entity));
                        }
                    }
                    Some(KeyCode::Escape) => {
                        if controller.triggers.has(ControllerFlag::BUILD) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        }
                    }
                    Some(KeyCode::A) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.rotations.set(ControllerFlag::LEFT);
                    }
                    Some(KeyCode::D) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.rotations.set(ControllerFlag::RIGHT);
                    }
                    Some(KeyCode::W) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.set(ControllerFlag::FORWARD);
                    }
                    Some(KeyCode::S) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.set(ControllerFlag::BACKWARD);
                    }
                    Some(KeyCode::N) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.directions.set(ControllerFlag::BACKWARD);
                    }
                    Some(KeyCode::Space) => {
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
                    Some(KeyCode::A) => {
                        controller.rotations.unset(ControllerFlag::LEFT);
                    }
                    Some(KeyCode::D) => {
                        controller.rotations.unset(ControllerFlag::RIGHT);
                    }
                    Some(KeyCode::W) => {
                        controller.directions.unset(ControllerFlag::FORWARD);
                    }
                    Some(KeyCode::S) => {
                        controller.directions.unset(ControllerFlag::BACKWARD);
                    }
                    Some(KeyCode::Left) => {
                        change_build_indicator.send(ChangeBuildIndicator(entity, -1));
                    }
                    Some(KeyCode::Right) => {
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

fn key_for_input(key_code: Option<KeyCode>) -> AnimationKey {
    match key_code {
        Some(KeyCode::Key1) => AnimationKey::Idle, //OK
        Some(KeyCode::Key2) => AnimationKey::Walk, //OK
        Some(KeyCode::Key3) => AnimationKey::Yes, //OK
        Some(KeyCode::Key4) => AnimationKey::Wave, //OK
        Some(KeyCode::Key5) => AnimationKey::RunGun,
        Some(KeyCode::Key6) => AnimationKey::Run,
        Some(KeyCode::Key7) => AnimationKey::Punch,
        Some(KeyCode::Key8) => AnimationKey::No, //OK
        Some(KeyCode::Key9) => AnimationKey::JumpLand, //Punch?
        Some(KeyCode::Key0) => AnimationKey::JumpIdle, //JumpMidAir, JumpIdle, OK?
        Some(KeyCode::Numpad0) => AnimationKey::Jump, //Run
        Some(KeyCode::Numpad1) => AnimationKey::IdleShoot, //OK
        Some(KeyCode::Numpad2) => AnimationKey::HitReact, //OK
        Some(KeyCode::Numpad3) => AnimationKey::Duck, //OK
        Some(KeyCode::Numpad4) => AnimationKey::Death, //OK
        Some(KeyCode::Numpad5) => AnimationKey::WalkShoot, //OK
        Some(KeyCode::Numpad6) => AnimationKey::RunShoot, //OK
        _ => AnimationKey::Idle
    }
}