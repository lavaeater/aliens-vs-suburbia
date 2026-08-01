use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{Entity, MessageReader, MessageWriter, KeyCode, Query, Res, ResMut, With, Without};
use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::control::components::{CharacterControl, ControlCommand, ControlDirection, InputKeyboard};
use crate::control::gamepad_input::stick_to_world;
use crate::settings::resources::GameSettings;
use crate::player::components::PlayerDead;
use crate::player::events::building_events::{ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode};
use crate::player::systems::abilities::AbilityInput;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn keyboard_input(
    mut key_evr: MessageReader<KeyboardInput>,
    mut query: Query<(Entity, &mut CharacterControl), (With<InputKeyboard>, Without<PlayerDead>)>,
    settings: Res<GameSettings>,
    mut start_build_ew: MessageWriter<EnterBuildMode>,
    mut execute_build: MessageWriter<ExecuteBuild>,
    mut exit_build: MessageWriter<ExitBuildMode>,
    mut change_build_indicator: MessageWriter<ChangeBuildIndicator>,
    mut animation_ew: MessageWriter<AnimationEvent>,
    mut ability_input: Option<ResMut<AbilityInput>>,
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
                    KeyCode::Escape
                        if controller.triggers.contains(&ControlCommand::Build) => {
                            animation_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Building));
                            exit_build.write(ExitBuildMode(entity));
                        }
                    KeyCode::KeyA => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.insert(ControlDirection::Left);
                    }
                    KeyCode::KeyD => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.insert(ControlDirection::Right);
                    }
                    KeyCode::KeyW => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.insert(ControlDirection::Forward);
                    }
                    KeyCode::KeyS => {
                        animation_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, entity, AnimationKey::Walk));
                        controller.directions.insert(ControlDirection::Backward);
                    }
                    KeyCode::Space => {
                        if controller.triggers.contains(&ControlCommand::Build) {
                            execute_build.write(ExecuteBuild(entity));
                        } else {
                            controller.triggers.insert(ControlCommand::Throw);
                        }
                    }
                    KeyCode::KeyQ => {
                        if let Some(ref mut ai) = ability_input {
                            ai.pressed = true;
                        }
                    }
                    _ => {}
                },
                ButtonState::Released => match ev.key_code {
                    KeyCode::KeyA => {
                        controller.directions.remove(&ControlDirection::Left);
                    }
                    KeyCode::KeyD => {
                        controller.directions.remove(&ControlDirection::Right);
                    }
                    KeyCode::KeyW => {
                        controller.directions.remove(&ControlDirection::Forward);
                    }
                    KeyCode::KeyS => {
                        controller.directions.remove(&ControlDirection::Backward);
                    }
                    KeyCode::Space => {
                        controller.triggers.remove(&ControlCommand::Throw);
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
                animation_ew.write(AnimationEvent(AnimationEventType::LeaveAnimState, entity, AnimationKey::Walk));
            }

            // WASD is camera-relative, like the gamepad's left stick: W walks up the
            // screen whichever way the character is facing. The body is steered by
            // `face_movement_direction` and the torso twists to the mouse on top of
            // that, so A/D strafe rather than rotate.
            let mut stick = Vec2::ZERO;
            if controller.directions.contains(&ControlDirection::Forward) {
                stick.y += 1.0;
            }
            if controller.directions.contains(&ControlDirection::Backward) {
                stick.y -= 1.0;
            }
            if controller.directions.contains(&ControlDirection::Left) {
                stick.x -= 1.0;
            }
            if controller.directions.contains(&ControlDirection::Right) {
                stick.x += 1.0;
            }
            controller.walk_direction = stick_to_world(stick.normalize_or_zero(), settings.yaw_degrees);
            controller.torque = Vec3::ZERO;
        }
    }
}
