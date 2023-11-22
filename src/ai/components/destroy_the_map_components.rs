use bevy::prelude::Component;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};
use crate::general::components::map_components::CoolDown;

pub enum MustDestroyTheMapState {
    NotStarted,
    SearchingForThingToDestroy,
    MovingTowardsThingToDestroy,
    DestroyingThing,
    Finished,
    Failed
}

#[derive(Component)]
pub struct MustDestroyTheMap {
    pub path_of_destruction: Option<Vec<(usize, usize)>>,
    pub state: MustDestroyTheMapState,
    pub target_tile: Option<(usize, usize)>,
    pub attack_cooldown: f32,
    pub attack_rate_per_minute: f32
}

impl MustDestroyTheMap {
    pub fn new() -> Self {
        Self {
            path_of_destruction: None,
            state: MustDestroyTheMapState::NotStarted,
            target_tile: None,
            attack_cooldown: 0.0,
            attack_rate_per_minute: 30.0
        }
    }
}

impl CoolDown for MustDestroyTheMap {
    fn cool_down(&mut self, delta_seconds: f32) -> bool {
        self.attack_cooldown -= delta_seconds;
        if self.attack_cooldown <= 0.0 {
            self.attack_cooldown = 60.0 / self.attack_rate_per_minute;
            true
        } else {
            false
        }
    }
}


#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct DestroyTheMapScore;

#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct DestroyTheMapAction {}

