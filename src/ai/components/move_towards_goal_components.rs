use bevy::prelude::*;

#[derive(Clone, Component, Debug, Reflect, Default)]
#[reflect(Component, Default)]
#[type_path = "aliensvssuburbia"]
pub struct MoveTowardsGoalData {
    pub path: Option<Vec<(usize, usize)>>,
}

#[derive(Message, Clone)]
pub struct AgentReachedGoal(pub Entity);

#[derive(Message, Clone)]
pub struct AgentCannotFindPath(pub Entity);
