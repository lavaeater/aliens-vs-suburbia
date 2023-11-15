use bevy::prelude::{Component};
use bevy::reflect::Reflect;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};

#[derive(Clone, Component, Debug, Reflect)]
pub struct MoveTowardsGoalData {
    pub path: Option<Vec<(usize, usize)>>,
}

#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct MoveTowardsGoalScore;

#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct MoveTowardsGoalAction {}



