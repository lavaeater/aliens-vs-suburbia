use bevy::prelude::{Component, Entity};
use bevy::reflect::Reflect;

#[derive(Clone, Component, Debug, Reflect)]
pub struct ApproachAndAttackPlayerData {
    pub seen_player: Option<Entity>,
    pub attack_distance: f32,
}

impl Default for ApproachAndAttackPlayerData {
    fn default() -> Self {
        Self {
            seen_player: None,
            attack_distance: 0.5,
        }
    }
}
