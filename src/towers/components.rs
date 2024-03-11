use crate::general::components::map_components::CoolDown;
use bevy::prelude::{Component, Entity};
use bevy::utils::HashSet;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};

#[derive(Component)]
pub struct TowerSensor {}

#[derive(Component)]
pub struct TowerShooter {
    pub cool_down: f32,
    pub rate_of_fire_per_minute: f32,
}

impl TowerShooter {
    pub fn new(rate_of_fire_per_minute: f32) -> Self {
        Self {
            cool_down: 0.0,
            rate_of_fire_per_minute,
        }
    }
}

impl CoolDown for TowerShooter {
    fn cool_down(&mut self, delta_seconds: f32) -> bool {
        self.cool_down -= delta_seconds;
        if self.cool_down <= 0.0 {
            self.cool_down = 60.0 / self.rate_of_fire_per_minute;
            true
        } else {
            false
        }
    }
}

#[derive(Component)]
pub struct ShootAlienData {
    pub aliens_in_range: HashSet<Entity>,
}

// Scorers are the same as in the thirst example.
#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct AlienInRangeScore;

/// An action where the actor moves to the closest water source
#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct ShootAlienAction;
