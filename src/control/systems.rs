use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{Entity, Event, EventReader, EventWriter, KeyCode, Query, With};
use crate::animation::animation_plugin::{AnimationKey, AnimationKeyUpdated};
use crate::control::components::{ControlCommands, ControlDirection, Controller, ControlRotation, KeyboardController};
use crate::player::events::building_events::{ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode};


pub fn input_control(
    mut key_evr: EventReader<KeyboardInput>,
    mut query: Query<(Entity, &mut Controller), With<KeyboardController>>,
    mut start_build_ew: EventWriter<EnterBuildMode>,
    mut execute_build: EventWriter<ExecuteBuild>,
    mut exit_build: EventWriter<ExitBuildMode>,
    mut change_build_indicator: EventWriter<ChangeBuildIndicator>,
    mut update_animation_key_handler: EventWriter<AnimationKeyUpdated>,
) {
    if let Ok((entity, mut controller)) = query.get_single_mut() {
        for ev in key_evr.read() {
            match ev.state {
                ButtonState::Pressed => match ev.key_code {
                    Some(KeyCode::B) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Idle));
                            exit_build.send(ExitBuildMode(entity));
                        } else {
                            controller.triggers.insert(ControlCommands::Build);
                            update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Building));
                            start_build_ew.send(EnterBuildMode(entity));
                        }
                    }
                    Some(KeyCode::Escape) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Idle));
                            exit_build.send(ExitBuildMode(entity));
                        }
                    }
                    Some(KeyCode::A) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Left);
                    }
                    Some(KeyCode::D) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Right);
                    }
                    Some(KeyCode::W) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Forward);
                    }
                    Some(KeyCode::S) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Backward);
                    }
                    Some(KeyCode::Space) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            execute_build.send(ExecuteBuild(entity));
                        } else if controller.triggers.contains(&ControlCommands::Throw) {
                            controller.triggers.remove(&ControlCommands::Throw);
                        } else {
                            update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Throwing));
                            controller.triggers.insert(ControlCommands::Throw);
                        }
                    }
                    _ => {}
                },
                ButtonState::Released => match ev.key_code {
                    Some(KeyCode::A) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Idle));
                        controller.rotations.remove(&ControlRotation::Left);
                    }
                    Some(KeyCode::D) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Idle));
                        controller.rotations.remove(&ControlRotation::Right);
                    }
                    Some(KeyCode::W) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Idle));
                        controller.directions.remove(&ControlDirection::Forward);
                    }
                    Some(KeyCode::S) => {
                        update_animation_key_handler.send(AnimationKeyUpdated(entity, AnimationKey::Idle));
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
        }
    }
}