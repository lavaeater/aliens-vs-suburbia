use bevy::prelude::{Entity, Event};

#[derive(Event)]
pub struct StartBuilding(pub Entity);

#[derive(Event)]
pub struct StopBuilding(pub Entity);