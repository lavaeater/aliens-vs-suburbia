use bevy::math::Vec3;
use bevy::prelude::Event;

#[derive(Event)]
pub struct BuildTower {
    pub position: Vec3,
    pub model_definition_key: &'static str,
    
}