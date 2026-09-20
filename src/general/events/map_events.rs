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
    /// Roster slot to spawn into. `None` = next free slot (the map's spawn points).
    pub slot: Option<usize>,
    /// Lives to spawn with. `None` = `GameSettings::lives_per_player`.
    pub lives: Option<u32>,
}

#[derive(Message, Clone)]
pub struct SpawnAlien {
    pub position: Vec3,
}
