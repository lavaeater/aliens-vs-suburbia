use bevy::app::{App, Plugin, Update};
use bevy::input::{Axis, Input};
use bevy::input::gamepad::GamepadEvent;
use bevy::prelude::{Commands, Component, Entity, EventReader, Gamepad, GamepadAxis, GamepadAxisType, GamepadButton, GamepadButtonType, Query, Res, Resource, Without};
use bevy::utils::HashSet;
use crate::control::components::{CharacterControl, InputKeyboard};

/// Simple resource to store the ID of the connected gamepad.
/// We need to know which gamepad to use for player input.
#[derive(Component)]
struct InputGamepad {
    gamepad: Gamepad,
    left_x: GamepadAxis,
    left_y: GamepadAxis,
    right_x: GamepadAxis,
    right_y: GamepadAxis,
    button_north: GamepadButton,
    button_east: GamepadButton,
    button_south: GamepadButton,
    button_west: GamepadButton,
    trigger_right: GamepadButton,
    trigger_left: GamepadButton,
}

impl InputGamepad {
    fn new(gamepad: Gamepad) -> Self {
        InputGamepad {
            gamepad,
            left_x: GamepadAxis::new(gamepad, GamepadAxisType::LeftStickX),
            left_y: GamepadAxis::new(gamepad, GamepadAxisType::LeftStickY),
            right_x: GamepadAxis::new(gamepad, GamepadAxisType::RightStickX),
            right_y: GamepadAxis::new(gamepad, GamepadAxisType::RightStickY),
            button_north: GamepadButton::new(gamepad, GamepadButtonType::North),
            button_east: GamepadButton::new(gamepad, GamepadButtonType::East),
            button_south: GamepadButton::new(gamepad, GamepadButtonType::South),
            button_west: GamepadButton::new(gamepad, GamepadButtonType::West),
            trigger_right: GamepadButton::new(gamepad, GamepadButtonType::RightTrigger),
            trigger_left: GamepadButton::new(gamepad, GamepadButtonType::LeftTrigger),
        }
    }
}

pub struct GamepadPlugin;
impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, gamepad_connections)
            .add_systems(gamepad_game_input);
    }
}


fn gamepad_connections(
    mut commands: Commands,
    mut gamepad_evr: EventReader<GamepadEvent>,
) {
    for ev in gamepad_evr.read() {
        // the ID of the gamepad
        match &ev {
            GamepadEvent::Connection(info) => {
                println!("New gamepad connected with ID: {:?}", info.gamepad.id);
                if info.connected() {
                }
            }
            // other events are irrelevant
            _ => {}
        }
    }
}

fn gamepad_game_input(
    axes: Res<Axis<GamepadAxis>>,
    buttons: Res<Input<GamepadButton>>,
    player_query: Query<(Entity, &mut CharacterControl, &InputGamepad)>
) {
    for (entity, mut controller, input_gamepad) in player_query.iter() {
        // do something with the gamepad
        if let (Some(x), Some(y)) = (axes.get(input_gamepad.left_x), axes.get(input_gamepad.left_y)) {
            if x.abs() > 0.1 || y.abs() > 0.1 {
                controller.directions.insert(ControlDirection::Forward);
            } else {
                controller.directions.remove(&ControlDirection::Forward);
            }

        }
    }
}