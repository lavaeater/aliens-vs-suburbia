use bevy::math::Vec3;
use bevy::prelude::Message;

#[derive(Message, Clone)]
pub struct BuildTower {
    pub position: Vec3,
    /// Index into `MapModelDefinitions::build_indicators`.
    pub option: usize,
}
