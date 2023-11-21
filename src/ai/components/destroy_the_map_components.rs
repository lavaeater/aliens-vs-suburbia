use bevy::prelude::Component;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};

pub enum MustDestroyTheMapState {
    NotStarted,
    SearchingForThingToDestroy,
    MovingTowardsThingToDestroy,
    DestroyingThing,
    Finished,
}

#[derive(Component, Debug)]
pub struct MustDestroyTheMap {
    pub path_of_destruction: Option<Vec<(usize, usize)>>,
    pub state: MustDestroyTheMapState,
}

impl MustDestroyTheMap {
    pub fn new() -> Self {
        Self {
            path_of_destruction: None,
            state: MustDestroyTheMapState::NotStarted,
        }
    }
}

#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct DestroyTheMapScore;

#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct DestroyTheMapAction {}

