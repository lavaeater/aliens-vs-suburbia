use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{EventReader, KeyCode, Query, With};
use crate::player::components::general::{Controller, Directions, KeyboardController, Rotations, Triggers};

pub fn enemy_control(
    mut key_evr: EventReader<KeyboardInput>,
    mut query: Query<&mut Controller, With<KeyboardController>>,
) {
    if let Ok(mut controller) = query.get_single_mut() {
        for ev in key_evr.read() {
            match ev.state {
                ButtonState::Pressed => match ev.key_code {
                    Some(KeyCode::A) => {
                        controller.rotations.insert(Rotations::Left);
                    }
                    Some(KeyCode::D) => {
                        controller.rotations.insert(Rotations::Right);
                    }
                    Some(KeyCode::W) => {
                        controller.directions.insert(Directions::Forward);
                    }
                    Some(KeyCode::S) => {
                        controller.directions.insert(Directions::Backward);
                    }
                    Some(KeyCode::Space) => {
                        controller.triggers.insert(Triggers::Throw);
                    }
                    _ => {}
                },
                ButtonState::Released => match ev.key_code {
                    Some(KeyCode::A) => {
                        controller.rotations.remove(&Rotations::Left);
                    }
                    Some(KeyCode::D) => {
                        controller.rotations.remove(&Rotations::Right);
                    }
                    Some(KeyCode::W) => {
                        controller.directions.remove(&Directions::Forward);
                    }
                    Some(KeyCode::S) => {
                        controller.directions.remove(&Directions::Backward);
                    }
                    Some(KeyCode::Space) => {
                        controller.triggers.remove(&Triggers::Throw);
                    }
                    _ => {}
                }
            }
        }
    }
}