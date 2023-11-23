use bevy::log::info;
use bevy::prelude::Component;
use bevy::reflect::Reflect;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};
use crate::control::components::{ControlRotation, Opposite};
use crate::general::components::map_components::CoolDown;

#[derive(Clone, Component, Debug, Reflect)]
pub struct AvoidWallsData {
    pub forward_distance: f32,
    pub left_distance: f32,
    pub right_distance: f32,
    pub max_forward_distance: f32,
    pub max_left_distance: f32,
    pub max_right_distance: f32,
    pub rotation_direction: ControlRotation,
    pub rotation_timer: f32,
    pub rotation_timer_max: f32,
    pub proto_val: f32
}

impl CoolDown for AvoidWallsData {
    fn cool_down(&mut self, delta: f32) -> bool {
        self.rotation_timer -= delta;
        if self.rotation_timer <= 0.0 {
            self.rotation_direction = self.rotation_direction.opposite();
            info!("Timer expired, new direction is: {:?}", self.rotation_direction);
            self.rotation_timer = self.rotation_timer_max;
            true
        } else {
            false
        }
    }
}

impl AvoidWallsData {
    pub fn new(max_forward_distance: f32, max_left_distance: f32, max_right_distance: f32, rotation_timer: f32) -> Self {
        Self {
            forward_distance: max_forward_distance,
            left_distance: max_left_distance,
            max_left_distance,
            right_distance: max_right_distance,
            max_right_distance,
            max_forward_distance,
            rotation_direction: ControlRotation::Left,
            rotation_timer,
            rotation_timer_max: rotation_timer,
            proto_val: 0.0
        }
    }
}

// Scorers are the same as in the thirst example.
#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct AvoidWallScore;

/// An action where the actor moves to the closest water source
#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct AvoidWallsAction {}
