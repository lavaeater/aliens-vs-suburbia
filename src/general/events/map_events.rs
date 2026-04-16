use bevy::prelude::Message;
use bevy::math::Vec3;

#[derive(Message, Clone)]
pub struct LoadMap {}

#[derive(Message, Clone)]
pub struct SpawnPlayer {
    pub position: Vec3,
}

#[derive(Message, Clone)]
pub struct SpawnAlien {
    pub position: Vec3,
}
