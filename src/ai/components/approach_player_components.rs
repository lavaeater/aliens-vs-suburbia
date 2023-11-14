use bevy::prelude::{Component, Entity};
use big_brain::prelude::{ActionBuilder, ScorerBuilder};

#[derive(Clone, Component, Debug, Default)]
pub struct ApproachPlayerData {
    pub seen_player: Option<Entity>,
}

// Scorers are the same as in the thirst example.
#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct ApproachPlayerScore;

/// An action where the actor moves to the closest water source
#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct ApproachPlayerAction {}
