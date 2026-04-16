use bevy::app::{App, Plugin, Update};
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::{ButtonInput, Commands, Component, Entity, MessageReader, in_state, IntoScheduleConfigs, Query, Reflect, Res, With};
use bevy::input::gamepad::GamepadConnectionEvent;
use crate::control::components::{CharacterControl, InputKeyboard};
use crate::game_state::GameState;
use crate::player::components::Player;

/// Marker component for entities controlled by a gamepad.
#[derive(Component, Reflect)]
pub struct InputGamepad {
    pub gamepad: Entity,
}

impl InputGamepad {
    fn new(gamepad: Entity) -> Self {
        InputGamepad { gamepad }
    }
}

pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            gamepad_connection,
            gamepad_game_input,
        ).run_if(in_state(GameState::InGame)));
    }
}

fn gamepad_connection(
    mut connection_evr: MessageReader<GamepadConnectionEvent>,
    mut player_query: Query<Entity, With<Player>>,
    mut commands: Commands,
) {
    for event in connection_evr.read() {
        if event.connected() {
            if let Ok(entity) = player_query.single_mut() {
                commands.entity(entity).remove::<InputKeyboard>();
                commands.entity(entity).insert(InputGamepad::new(event.gamepad));
            }
        }
    }
}

fn gamepad_game_input(
    gamepads: Query<&Gamepad>,
    mut player_query: Query<(&mut CharacterControl, &InputGamepad)>,
) {
    for (mut controller, input_gamepad) in player_query.iter_mut() {
        if let Ok(gamepad) = gamepads.get(input_gamepad.gamepad) {
            let x = gamepad.left_stick().x;
            let y = gamepad.left_stick().y;

            if x.abs() > 0.1 {
                controller.walk_direction.x = x;
            } else {
                controller.walk_direction.x = 0.0;
            }
            if y.abs() > 0.1 {
                controller.walk_direction.z = -y;
            } else {
                controller.walk_direction.z = 0.0;
            }
        }
    }
}
