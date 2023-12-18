use bevy::input::{Axis, Input};
use bevy::input::gamepad::GamepadEvent;
use bevy::prelude::{Commands, EventReader, Gamepad, GamepadAxis, GamepadButton, Res, Resource};

/// Simple resource to store the ID of the connected gamepad.
/// We need to know which gamepad to use for player input.
#[derive(Resource)]
struct InputGamepad(Gamepad);

fn gamepad_connections(
    mut commands: Commands,
    gamepads: Option<Res<InputGamepad>>,
    mut gamepad_evr: EventReader<GamepadEvent>,
) {
    for ev in gamepad_evr.read() {
        // the ID of the gamepad
        match &ev {
            GamepadEvent::Connection(info) => {
                println!("New gamepad connected with ID: {:?}", info.gamepad.id);
                if info.connected() {
                    // store the ID of the gamepad
                    commands.insert_resource(InputGamepad(info.gamepad.clone()));
                } else {
                    // remove the resource
                    commands.remove_resource::<InputGamepad>();
                }
            }
            // other events are irrelevant
            _ => {}
        }
    }
}

fn gamepad_input(
    axes: Res<Axis<GamepadAxis>>,
    buttons: Res<Input<GamepadButton>>,
    my_gamepad: Option<Res<ControlGamepad>>,
) {
    let gamepad = if let Some(gp) = my_gamepad {
        // a gamepad is connected, we have the id
        gp.0
    } else {
        // no gamepad is connected
        return;
    };

    // The joysticks are represented using a separate axis for X and Y
    let axis_lx = GamepadAxis {
        gamepad, axis_type: GamepadAxisType::LeftStickX
    };
    let axis_ly = GamepadAxis {
        gamepad, axis_type: GamepadAxisType::LeftStickY
    };

    if let (Some(x), Some(y)) = (axes.get(axis_lx), axes.get(axis_ly)) {
        // combine X and Y into one vector
        let left_stick_pos = Vec2::new(x, y);

        // Example: check if the stick is pushed up
        if left_stick_pos.length() > 0.9 && left_stick_pos.y > 0.5 {
            // do something
        }
    }

    // In a real game, the buttons would be configurable, but here we hardcode them
    let jump_button = GamepadButton {
        gamepad, button_type: GamepadButtonType::South
    };
    let heal_button = GamepadButton {
        gamepad, button_type: GamepadButtonType::East
    };

    if buttons.just_pressed(jump_button) {
        // button just pressed: make the player jump
    }

    if buttons.pressed(heal_button) {
        // button being held down: heal the player
    }
}