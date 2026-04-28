use bevy::prelude::{Component, Resource};
use bevy::reflect::Reflect;
use std::collections::HashMap;
use avian3d::prelude::{CollisionLayers, LayerMask, RigidBody};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct DecorationItem {
    pub x: i32,
    pub y: i32,
    /// Path relative to `assets/`, e.g. `"packs/nature/Pine.glb"`. `#Scene0` is appended automatically.
    pub model: String,
    pub rotation_y: f32,
    #[serde(default = "default_scale")]
    pub scale: f32,
}

fn default_scale() -> f32 { 1.0 }

#[derive(Deserialize, Clone, Debug, Default)]
pub struct MapFile {
    /// When true, `tiles` and `decorations` are ignored and the map is procedurally generated from `seed`.
    #[serde(default)]
    pub generated: bool,
    #[serde(default)]
    pub seed: u64,
    /// Row-major grid; 0=void, 1=floor, 5=alien spawn, 9=alien goal, 17=player spawn.
    #[serde(default)]
    pub tiles: Vec<Vec<u8>>,
    #[serde(default)]
    pub decorations: Vec<DecorationItem>,
}

#[derive(Component)]
pub struct Wall {}

#[derive(Component)]
pub struct AlienGoal {
    pub x: usize,
    pub y: usize
}

pub struct ModelDefinition {
    pub name: &'static str,
    pub file: &'static str,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub rigid_body: RigidBody,
    pub group: LayerMask,
    pub mask: LayerMask,
}

#[derive(Hash, PartialEq, Eq, Clone, Reflect,Component)]
pub struct Tower {}

impl ModelDefinition {
    pub fn create_collision_layers(&self) -> CollisionLayers {
        CollisionLayers::new(self.group, self.mask)
    }
}

#[derive(Resource)]
pub struct MapModelDefinitions {
    pub definitions: HashMap<&'static str, ModelDefinition>,
    pub build_indicators: Vec<&'static str>,
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
