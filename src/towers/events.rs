use bevy::math::Vec3;
use bevy::prelude::Event;
use crate::general::components::map_components::ModelDefinition;

#[derive(Event)]
pub struct BuildTower {
    pub position: Vec3,
    pub model_definition_key: &'static str,
    
}