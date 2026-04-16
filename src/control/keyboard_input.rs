use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::math::Vec3;
use bevy::prelude::{Entity, MessageReader, MessageWriter, KeyCode, Query, With};
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::components::{CharacterControl, ControlCommand, ControlDirection, ControlRotation, InputKeyboard};
use crate::player::events::building_events::{ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode};

pub fn keyboard_input(
    mut key_evr: MessageReader<KeyboardInput>,
    mut query: Query<(Entity, &mut CharacterControl), With<InputKeyboard>>,
    mut start_build_ew: MessageWriter<EnterBuildMode>,
    mut execute_build: MessageWriter<ExecuteBuild>,
    mut exit_build: MessageWriter<ExitBuildMode>,
    mut change_build_indicator: MessageWriter<ChangeBuildIndicator>,
    mut animation_ew: MessageWriter<AnimationEvent>,
) {
    if let Ok((entity, mut controller)) = query.single_mut() {
        for ev in key_evr.read() {
            match ev.state {
                ButtonState::Pressed => match ev.key_code {
                    KeyCode::KeyB => {
                        if controller.triggers.contains(&ControlCommand::Build) {
                            animation_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.write(ExitBuildMode(entity));
                        } else {
                            controller.triggers.insert(ControlCommand::Build);
                            animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Building));
                            start_build_ew.write(EnterBuildMode(entity));
                        }
                    }
                    KeyCode::Escape => {
                        if controller.triggers.contains(&ControlCommand::Build) {
                            animation_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.write(ExitBuildMode(entity));
                        }
                    }
                    KeyCode::KeyA => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Left);
                    }
                    KeyCode::KeyD => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Right);
                    }
                    KeyCode::KeyW => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Forward);
                    }
                    KeyCode::KeyS => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Backward);
                    }
                    KeyCode::Space => {
                        if controller.triggers.contains(&ControlCommand::Build) {
                            execute_build.write(ExecuteBuild(entity));
                        } else if controller.triggers.contains(&ControlCommand::Throw) {
                            animation_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Throwing));
                            controller.triggers.remove(&ControlCommand::Throw);
                        } else {
                            animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Throwing));
                            controller.triggers.insert(ControlCommand::Throw);
                        }
                    }
                    _ => {}
                },
                ButtonState::Released => match ev.key_code {
                    KeyCode::KeyA => {
                        controller.rotations.remove(&ControlRotation::Left);
                    }
                    KeyCode::KeyD => {
                        controller.rotations.remove(&ControlRotation::Right);
                    }
                    KeyCode::KeyW => {
                        controller.directions.remove(&ControlDirection::Forward);
                    }
                    KeyCode::KeyS => {
                        controller.directions.remove(&ControlDirection::Backward);
                    }
                    KeyCode::ArrowLeft => {
                        change_build_indicator.write(ChangeBuildIndicator(entity, -1));
                    }
                    KeyCode::ArrowRight => {
                        change_build_indicator.write(ChangeBuildIndicator(entity, 1));
                    }
                    _ => {}
                }
            }
            if controller.directions.is_empty() && controller.rotations.is_empty() {
                animation_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Walking));
            }

            controller.walk_direction = Vec3::ZERO;
            controller.torque = Vec3::ZERO;

            if controller.directions.contains(&ControlDirection::Forward) {
                controller.walk_direction.z = -1.0;
            }
            if controller.directions.contains(&ControlDirection::Backward) {
                controller.walk_direction.z = 1.0;
            }
            if controller.rotations.contains(&ControlRotation::Left) {
                controller.torque.y = 1.0;
            }
            if controller.rotations.contains(&ControlRotation::Right) {
                controller.torque.y = -1.0;
            }
        }
    }
}
