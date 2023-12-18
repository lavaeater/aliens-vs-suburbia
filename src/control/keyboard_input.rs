use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{Entity, EventReader, EventWriter, KeyCode, Query, With};
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::components::{CharacterControl, ControlCommands, ControlDirection, ControlRotation, InputKeyboard};
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
                        if controller.triggers.contains(&ControlCommands::Build) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        } else {
                            controller.triggers.insert(ControlCommands::Build);
                            animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Building));
                            start_build_ew.send(EnterBuildMode(entity));
                        }
                    }
                    Some(KeyCode::Escape) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        }
                    }
                    Some(KeyCode::A) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Left);
                    }
                    Some(KeyCode::D) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Right);
                    }
                    Some(KeyCode::W) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Forward);
                    }
                    Some(KeyCode::S) => {
                        animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Backward);
                    }
                    Some(KeyCode::Space) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            execute_build.send(ExecuteBuild(entity));
                        } else if controller.triggers.contains(&ControlCommands::Throw) {
                            animation_ew.send(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Throwing));
                            controller.triggers.remove(&ControlCommands::Throw);
                        } else {
                            animation_ew.send(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Throwing));
                            controller.triggers.insert(ControlCommands::Throw);
                        }
                    }
                    _ => {}
                },
                ButtonState::Released => match ev.key_code {
                    Some(KeyCode::A) => {
                        controller.rotations.remove(&ControlRotation::Left);
                    }
                    Some(KeyCode::D) => {
                        controller.rotations.remove(&ControlRotation::Right);
                    }
                    Some(KeyCode::W) => {
                        controller.directions.remove(&ControlDirection::Forward);
                    }
                    Some(KeyCode::S) => {
                        controller.directions.remove(&ControlDirection::Backward);
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
        }
    }
}