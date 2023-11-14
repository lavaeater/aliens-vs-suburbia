pub (crate) mod map_components;

use bevy::prelude::{Component, Reflect};
use bevy_xpbd_3d::prelude::PhysicsLayer;

#[derive(Component)]
pub struct Ball {
    pub bounces: i32,
    pub bounce_max: i32,
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            bounces: 0,
            bounce_max: 5,
        }
    }
}

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
    pub damage_range: i32,
}

impl Default for Attack {
    fn default() -> Self {
        Self {
            damage_range: 5,
        }
    }
}


#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct Health {
    pub health: i32,
    pub max_health: i32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            health: 100,
            max_health: 100,
        }
    }
}


