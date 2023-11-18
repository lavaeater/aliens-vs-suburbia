use bevy::prelude::{Component, Resource};
use bevy::reflect::Reflect;
use bevy::utils::HashMap;
use bevy_xpbd_3d::components::{CollisionLayers, RigidBody};
use bevy_xpbd_3d::prelude::PhysicsLayer;
use crate::general::components::CollisionLayer;

#[derive(Component)]
pub struct Wall {}

#[derive(Component)]
pub struct AlienGoal {
    pub x: usize,
    pub y: usize
}

pub struct ModelDefinition<L: PhysicsLayer> {
    pub file: &'static str,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub rigid_body: RigidBody,
    pub group: Vec<L>,
    pub mask: Vec<L>,
}

impl ModelDefinition<CollisionLayer> {
    pub fn create_collision_layers(&self) -> CollisionLayers {
        CollisionLayers::new(self.group.clone(), self.mask.clone())
    }
}

#[derive(Resource)]
pub struct ModelDefinitions<L:PhysicsLayer> {
    pub definitions: HashMap<&'static str, ModelDefinition<L>>
}

impl AlienGoal {
    pub fn new(x: usize, y: usize) -> Self {
        Self {
            x,
            y
        }
    }
}

#[derive(Component, Debug, Reflect, Default)]
pub struct CurrentTile {
    pub tile: (usize, usize)
}

#[derive(Component, Debug, Reflect)]
pub struct AlienSpawnPoint {
    pub spawn_rate_per_minute: f32,
    pub spawn_cool_down: f32
}

impl AlienSpawnPoint {
    pub fn new(spawn_rate_per_minute: f32) -> Self {
        Self {
            spawn_rate_per_minute,
            spawn_cool_down: 0.0
        }
    }
}

pub trait CoolDown {
    /// Returns true if the cool down is finished
    fn cool_down(&mut self, delta: f32) -> bool;
}

impl CoolDown for AlienSpawnPoint {
    fn cool_down(&mut self, delta: f32) -> bool {
        self.spawn_cool_down -= delta;
        if self.spawn_cool_down <= 0.0 {
            self.spawn_cool_down = 60.0 / self.spawn_rate_per_minute;
            return true;
        }
        false
    }
}


#[derive(Component)]
pub struct Floor {}
