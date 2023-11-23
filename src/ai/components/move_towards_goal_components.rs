use bevy::prelude::{Component, Entity, Event};
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

#[derive(Event)]
pub struct AgentReachedGoal(pub Entity);

#[derive(Event)]
pub struct AgentCannotFindPath(pub Entity);
