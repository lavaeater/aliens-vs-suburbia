use bevy::prelude::{Component, Entity};
use bevy::utils::HashSet;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};

#[derive(Component)]
pub struct TowerShootyBit {}

#[derive(Component)]
pub struct ShootAlienData {
    pub aliens_in_range: HashSet<Entity>
}


// Scorers are the same as in the thirst example.
#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct AlienInRangeScore;

/// An action where the actor moves to the closest water source
#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct ShootAlienAction;
