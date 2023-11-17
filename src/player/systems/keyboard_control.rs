use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{Entity, EventReader, EventWriter, KeyCode, Query, With};
use crate::player::components::general::{Controller, ControlDirection, KeyboardController, ControlRotation, ControlCommands};
use crate::player::events::building_events::{EnterBuildMode, ExitBuildMode, ExecuteBuild};

pub fn input_control(
    mut key_evr: EventReader<KeyboardInput>,
    mut query: Query<(Entity, &mut Controller), With<KeyboardController>>,
    mut start_build_ew: EventWriter<EnterBuildMode>,
    mut execute_build: EventWriter<ExecuteBuild>,
    mut cancel_build: EventWriter<ExitBuildMode>,
) {
    if let Ok((entity, mut controller)) = query.get_single_mut() {
        for ev in key_evr.read() {
            match ev.state {
                ButtonState::Pressed => match ev.key_code {
                    Some(KeyCode::B) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            execute_build.send(ExecuteBuild(entity));
                        } else {
                            controller.triggers.insert(ControlCommands::Build);
                            start_build_ew.send(EnterBuildMode(entity));
                        }
                    }
                    Some(KeyCode::Escape) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            cancel_build.send(ExitBuildMode(entity));
                        }
                    }
                    Some(KeyCode::A) => {
                        controller.rotations.insert(ControlRotation::Left);
                    }
                    Some(KeyCode::D) => {
                        controller.rotations.insert(ControlRotation::Right);
                    }
                    Some(KeyCode::W) => {
                        controller.directions.insert(ControlDirection::Forward);
                    }
                    Some(KeyCode::S) => {
                        controller.directions.insert(ControlDirection::Backward);
                    }
                    Some(KeyCode::Space) => {
                        if controller.triggers.contains(&ControlCommands::Throw) {
                            controller.triggers.remove(&ControlCommands::Throw);
                        } else {
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
                    _ => {}
                }
            }
        }
    }
}