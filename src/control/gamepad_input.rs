use bevy::app::{App, Plugin, Update};
use bevy::input::{Axis, Input};
use bevy::input::gamepad::GamepadButtonInput;
use bevy::prelude::{Commands, Component, Entity, EventReader, Gamepad, GamepadAxis, GamepadAxisType, GamepadButton, GamepadButtonType, in_state, IntoSystemConfigs, Query, Res, With};
use crate::control::components::{CharacterControl, InputKeyboard};
use crate::game_state::GameState;
use crate::player::components::Player;

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
            .add_systems(Update, (
                gamepad_game_input,
                gamepad_buttons
            )
                .run_if(in_state(GameState::InGame)))
        ;
    }
}

fn gamepad_buttons(
    mut gamepad_button_er: EventReader<GamepadButtonInput>,
    mut player_query: Query<Entity, With<Player>>,
    mut commands: Commands,
) {
    for event in gamepad_button_er.read() {
        if event.button.button_type == GamepadButtonType::Start {
            if let Ok(entity) = player_query.get_single_mut() {
                commands.entity(entity).remove::<InputKeyboard>();
                commands.entity(entity).insert(InputGamepad::new(event.button.gamepad));
            }
        }
    }
}

fn gamepad_game_input(
    axes: Res<Axis<GamepadAxis>>,
    buttons: Res<Input<GamepadButton>>,
    mut player_query: Query<(Entity, &mut CharacterControl, &InputGamepad)>
) {
    for (entity, mut controller, input_gamepad) in player_query.iter_mut() {
        // do something with the gamepad
        if let (Some(x), Some(y)) = (axes.get(input_gamepad.left_x), axes.get(input_gamepad.left_y)) {
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