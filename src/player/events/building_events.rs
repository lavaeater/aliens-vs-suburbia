use bevy::prelude::{Entity, Message};

#[derive(Message, Clone)]
pub struct EnterBuildMode(pub Entity);

#[derive(Message, Clone)]
pub struct ExecuteBuild(pub Entity);

#[derive(Message, Clone)]
pub struct RemoveTile(pub (usize, usize));

#[derive(Message, Clone)]
pub struct AddTile(pub (usize, usize));

#[derive(Message, Clone)]
pub struct ExitBuildMode(pub Entity);

#[derive(Message, Clone)]
pub struct ChangeBuildIndicator(pub Entity, pub i32); //negative for back, positive for forward

#[derive(Message, Clone)]
pub struct CurrentTileUpdated(pub Entity, pub (usize, usize));

#[derive(Message, Clone)]
pub struct BuildingIndicatorPositionUpdated(pub Entity, pub (usize, usize));
