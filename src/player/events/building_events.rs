use bevy::prelude::{Entity, Event};

#[derive(Event)]
pub struct EnterBuildMode(pub Entity);

#[derive(Event)]
pub struct ExecuteBuild(pub Entity);

#[derive(Event)]
pub struct RemoveTile(pub (usize, usize));

#[derive(Event)]
pub struct AddTile(pub (usize, usize));


#[derive(Event)]
pub struct ExitBuildMode(pub Entity);

#[derive(Event)]
pub struct ChangeBuildIndicator(pub Entity, pub i32); //negative for back, positive for forward

#[derive(Event)]
pub struct CurrentTileUpdated(pub Entity, pub (usize, usize));

#[derive(Event)]
pub struct BuildingIndicatorPositionUpdated(pub Entity, pub (usize, usize));