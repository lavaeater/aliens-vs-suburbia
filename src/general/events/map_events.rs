use bevy::prelude::Message;
use bevy::math::Vec3;
use crate::general::components::map_components::MapFile;

#[derive(Message, Clone)]
pub struct LoadMap {
    pub map: MapFile,
}

#[derive(Message, Clone)]
pub struct SpawnPlayer {
    pub position: Vec3,
}

#[derive(Message, Clone)]
pub struct SpawnAlien {
    pub position: Vec3,
}
