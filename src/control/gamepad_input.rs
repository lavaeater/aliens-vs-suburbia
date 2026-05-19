use bevy::app::{App, Plugin, Update};
use bevy::input::gamepad::Gamepad;
use bevy::prelude::{ Commands, Component, Entity, MessageReader, MessageWriter, in_state, IntoScheduleConfigs, Query, Reflect, With};
use bevy::input::gamepad::GamepadConnectionEvent;
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
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
        if event.connected()
            && let Ok(entity) = player_query.single_mut() {
                commands.entity(entity).remove::<InputKeyboard>();
                commands.entity(entity).insert(InputGamepad::new(event.gamepad));
            }
    }
}

fn gamepad_game_input(
    gamepads: Query<&Gamepad>,
    mut player_query: Query<(Entity, &mut CharacterControl, &InputGamepad)>,
    mut anim_ew: MessageWriter<AnimationEvent>,
) {
    for (entity, mut controller, input_gamepad) in player_query.iter_mut() {
        let Ok(gamepad) = gamepads.get(input_gamepad.gamepad) else { continue };

        let x = gamepad.left_stick().x;
        let y = gamepad.left_stick().y;
        let moving = x.abs() > 0.1 || y.abs() > 0.1;
        let was_moving = controller.walk_direction.length_squared() > 0.01;

        controller.walk_direction.x = if x.abs() > 0.1 { x } else { 0.0 };
        controller.walk_direction.z = if y.abs() > 0.1 { -y } else { 0.0 };

        if moving && !was_moving {
            anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
        } else if !moving && was_moving {
            anim_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Walk));
        }
    }
}
