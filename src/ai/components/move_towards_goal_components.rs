use bevy::prelude::{Component, Entity, Message};
use bevy::reflect::Reflect;

#[derive(Clone, Component, Debug, Reflect)]
pub struct MoveTowardsGoalData {
    pub path: Option<Vec<(usize, usize)>>,
}

#[derive(Message, Clone)]
pub struct AgentReachedGoal(pub Entity);

#[derive(Message, Clone)]
pub struct AgentCannotFindPath(pub Entity);
