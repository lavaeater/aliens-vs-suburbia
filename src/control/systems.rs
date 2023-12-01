use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{Component, Entity, EventReader, EventWriter, KeyCode, Query, With};
use crate::animation::animation_plugin::{AnimationKey, GotoAnimationState, LeaveAnimationState};
use crate::control::components::{ControlCommands, ControlDirection, Controller, ControlRotation, KeyboardController};
use crate::player::events::building_events::{ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode};

#[derive(Component)]
pub struct CharacterState {
    pub state: Vec<AnimationKey>,
}

impl CharacterState {
    pub fn enter_state(&mut self, state: AnimationKey) -> bool {
        if let Some(latest_state) = self.state.last() {
            if latest_state != &state {
                self.state.push(state);
                return true;
            }
        }
        false
    }

    pub fn leave_state(&mut self, state: AnimationKey) -> (bool, AnimationKey) {
        if self.state.len() > 1 {
            if let Some(latest_state) = self.state.last() {
                if latest_state == &state {
                    self.state.pop();
                    return (true, *self.state.last().unwrap());
                }
            }
        }
        (false, state)
    }
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            state: vec![AnimationKey::Idle]
        }
    }
}
pub fn input_control(
    mut key_evr: EventReader<KeyboardInput>,
    mut query: Query<(Entity, &mut Controller), With<KeyboardController>>,
    mut start_build_ew: EventWriter<EnterBuildMode>,
    mut execute_build: EventWriter<ExecuteBuild>,
    mut exit_build: EventWriter<ExitBuildMode>,
    mut change_build_indicator: EventWriter<ChangeBuildIndicator>,
    mut goto_animation_state_ew: EventWriter<GotoAnimationState>,
    mut leave_animation_state_ew: EventWriter<LeaveAnimationState>,
) {
    if let Ok((entity, mut controller)) = query.get_single_mut() {
        for ev in key_evr.read() {
            match ev.state {
                ButtonState::Pressed => match ev.key_code {
                    Some(KeyCode::B) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            leave_animation_state_ew.send(LeaveAnimationState(entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        } else {
                            controller.triggers.insert(ControlCommands::Build);
                            goto_animation_state_ew.send(GotoAnimationState(entity, AnimationKey::Building));
                            start_build_ew.send(EnterBuildMode(entity));
                        }
                    }
                    Some(KeyCode::Escape) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            leave_animation_state_ew.send(LeaveAnimationState(entity, AnimationKey::Building));
                            exit_build.send(ExitBuildMode(entity));
                        }
                    }
                    Some(KeyCode::A) => {
                        goto_animation_state_ew.send(GotoAnimationState(entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Left);
                    }
                    Some(KeyCode::D) => {
                        goto_animation_state_ew.send(GotoAnimationState(entity, AnimationKey::Walking));
                        controller.rotations.insert(ControlRotation::Right);
                    }
                    Some(KeyCode::W) => {
                        goto_animation_state_ew.send(GotoAnimationState(entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Forward);
                    }
                    Some(KeyCode::S) => {
                        goto_animation_state_ew.send(GotoAnimationState(entity, AnimationKey::Walking));
                        controller.directions.insert(ControlDirection::Backward);
                    }
                    Some(KeyCode::Space) => {
                        if controller.triggers.contains(&ControlCommands::Build) {
                            execute_build.send(ExecuteBuild(entity));
                        } else if controller.triggers.contains(&ControlCommands::Throw) {
                            leave_animation_state_ew.send(LeaveAnimationState(entity, AnimationKey::Throwing));
                            controller.triggers.remove(&ControlCommands::Throw);
                        } else {
                            goto_animation_state_ew.send(GotoAnimationState(entity, AnimationKey::Throwing));
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
                leave_animation_state_ew.send(LeaveAnimationState(entity, AnimationKey::Walking));
            }
        }
    }
}