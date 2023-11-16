use bevy::prelude::{Entity, Event};

#[derive(Event)]
pub struct StartBuilding(pub Entity);

#[derive(Event)]
pub struct StopBuilding(pub Entity);

#[derive(Event)]
pub struct CurrentTileUpdated(pub Entity, pub (usize, usize));

#[derive(Event)]
pub struct BuildingIndicatorPositionUpdated(pub Entity, pub (usize, usize));