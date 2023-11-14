use bevy::prelude::Component;
use bevy::reflect::Reflect;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};
use crate::player::components::general::ControlRotation;

#[derive(Clone, Component, Debug, Reflect)]
pub struct AvoidWallsData {
    pub forward_distance: f32,
    pub left_distance: f32,
    pub right_distance: f32,
    pub max_distance: f32,
    pub rotation_direction: ControlRotation,
    pub rotation_timer: f32,
    pub rotation_timer_max: f32,
    pub proto_val: f32
}

impl AvoidWallsData {
    pub fn new(max_distance: f32) -> Self {
        Self {
            forward_distance: max_distance,
            left_distance: max_distance,
            right_distance: max_distance,
            max_distance,
            rotation_direction: ControlRotation::Left,
            rotation_timer: 1.0,
            rotation_timer_max: 1.0,
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
