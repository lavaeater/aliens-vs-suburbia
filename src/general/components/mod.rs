pub (crate) mod map_components;

use bevy::prelude::{Component, Entity, Reflect};
use avian3d::prelude::PhysicsLayer;

/// Entities with this component deal damage per second to any Player they collide with.
#[derive(Component, Clone, Copy, Default, Reflect)]
 #[type_path = "aliensvssuburbia"]
pub struct TouchDamage {
    pub dps: f32,
}

/// Marker: this entity cannot be targeted or damaged by alien DestroyTheMap behaviour.
#[derive(Component, Clone, Copy, Default, Reflect)]
 #[type_path = "aliensvssuburbia"]
pub struct Indestructible;

#[derive(Component, Default, Reflect)]
 #[type_path = "aliensvssuburbia"]
pub struct Ball {
    pub entity: Entity,
    pub bounces: u32,
    pub max_bounces: u32,
    pub can_score: bool,
}

impl Ball {
    pub(crate) fn new(entity: Entity) -> Self {
        Self {
            entity,
            bounces: 0,
            max_bounces: 5,
            can_score: true,
        }
    }
}

#[derive(Component, Default, Reflect)]
 #[type_path = "aliensvssuburbia"]
pub struct HittableTarget {}

#[derive(PhysicsLayer, Default, PartialEq, Eq, Clone, Copy)]
pub enum CollisionLayer {
    #[default]
    Floor,
    Ball,
    Impassable,
    Alien,
    Player,
    AlienSpawnPoint,
    AlienGoal,
    BuildIndicator,
    Sensor,
    PlayerAimSensor,
}

#[derive(Component, Clone, Debug, PartialEq, Default, Reflect)]
 #[type_path = "aliensvssuburbia"]
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


#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect, Default, Reflect)]
 #[type_path = "aliensvssuburbia"]
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


