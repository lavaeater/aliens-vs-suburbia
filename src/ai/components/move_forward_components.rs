use bevy::prelude::Component;
use big_brain::prelude::{ActionBuilder, ScorerBuilder};

#[derive(Clone, Component, Debug, ScorerBuilder)]
pub struct MoveForwardScore;

#[derive(Clone, Component, Debug, ActionBuilder)]
pub struct MoveForwardAction {}
