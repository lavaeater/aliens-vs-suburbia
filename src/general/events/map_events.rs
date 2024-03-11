use bevy::math::Vec3;
use bevy::prelude::Event;

#[derive(Event)]
pub struct LoadMap {}

#[derive(Event)]
pub struct SpawnPlayer {
    pub position: Vec3,
}

#[derive(Event)]
pub struct SpawnAlien {
    pub position: Vec3,
}
