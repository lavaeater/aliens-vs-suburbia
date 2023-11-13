use bevy::prelude::Component;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};

#[derive(Clone, Component, Debug)]
pub struct AvoidWallsData {
    pub forward_distance: f32,
    pub left_distance: f32,
    pub right_distance: f32,
    pub max_distance: f32,
}

impl AvoidWallsData {
    pub fn new(max_distance: f32) -> Self {
        Self { forward_distance: max_distance, left_distance: max_distance, right_distance: max_distance, max_distance }
    }
}

// Scorers are the same as in the thirst example.
#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct AvoidWallScore;

/// An action where the actor moves to the closest water source
#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct AvoidWallsAction {}
