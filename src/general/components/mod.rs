use std::ops::{RangeInclusive};
use bevy::prelude::{Component, Reflect};
use bevy_xpbd_3d::prelude::PhysicsLayer;

#[derive(Component)]
pub struct Ball {}

#[derive(Component)]
pub struct Wall {}

#[derive(Component)]
pub struct Floor {}

#[derive(Component)]
pub struct HittableTarget {}

#[derive(PhysicsLayer)]
pub enum Layer {
    Floor,
    Ball,
    Wall,
    Alien,
    Player,
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct Attack {
    pub damage_range: RangeInclusive<f32>,
}

impl Default for Attack {
    fn default() -> Self {
        Self {
            damage_range: 5.0..=20.0,
        }
    }
}


#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Health {
    pub health: f32,
    pub max_health: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            health: 100.0,
            max_health: 100.0,
        }
    }
}


