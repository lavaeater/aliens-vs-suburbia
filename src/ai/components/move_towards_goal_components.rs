use bevy::prelude::*;

#[derive(Clone, Component, Debug, Reflect, Default)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct MoveTowardsGoalData {
    /// The tile being walked to, held until the alien gets there: choosing afresh on
    /// crossing into a tile would cut the corners of walls.
    pub next: Option<(usize, usize)>,
}

#[derive(Message, Clone)]
pub struct AgentReachedGoal(pub Entity);

#[derive(Message, Clone)]
pub struct AgentCannotFindPath(pub Entity);
