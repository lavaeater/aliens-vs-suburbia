use bevy::prelude::{Component, Entity, Event};
use bevy::reflect::Reflect;

#[derive(Clone, Component, Debug, Reflect)]
pub struct MoveTowardsGoalData {
    pub path: Option<Vec<(usize, usize)>>,
}

#[derive(Event)]
pub struct AgentReachedGoal(pub Entity);

#[derive(Event)]
pub struct AgentCannotFindPath(pub Entity);
