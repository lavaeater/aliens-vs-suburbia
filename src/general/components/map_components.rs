use bevy::prelude::{Component, Resource};
use bevy::reflect::Reflect;
use std::collections::HashMap;
use avian3d::prelude::{CollisionLayers, LayerMask, RigidBody};
use serde::{Deserialize, Serialize};

/// A user-placed model (from an AssetDefinition) on a specific grid tile.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TilePlacement {
    pub x: i32,
    pub y: i32,
    /// Path to the `.ron` def file, relative to project root.
    pub def_path: String,
    /// Rotation in 45° steps (0–7).
    #[serde(default)]
    pub rotation_steps: u8,
}

/// One wave definition: what enemy, how many, at what rate.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaveDef {
    /// Path to an Enemy-typed def file.
    pub enemy_def: String,
    pub count: u32,
    pub spawn_rate_per_minute: f32,
}

impl Default for WaveDef {
    fn default() -> Self {
        Self { enemy_def: String::new(), count: 10, spawn_rate_per_minute: 20.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

fn default_map_width() -> usize { 14 }
fn default_map_height() -> usize { 24 }

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MapFile {
    /// When true, `tiles` and `decorations` are ignored and the map is procedurally generated from `seed`.
    #[serde(default)]
    pub generated: bool,
    #[serde(default)]
    pub seed: u64,
    /// Procedural map dimensions. Only used when `generated: true`.
    #[serde(default = "default_map_width")]
    pub map_width: usize,
    #[serde(default = "default_map_height")]
    pub map_height: usize,
    /// Row-major grid; 0=void, 1=floor, 5=alien spawn, 9=alien goal, 17=player spawn.
    #[serde(default)]
    pub tiles: Vec<Vec<u8>>,
    #[serde(default)]
    pub decorations: Vec<DecorationItem>,
    /// Items placed by the map editor (def-driven models on specific tiles).
    #[serde(default)]
    pub placements: Vec<TilePlacement>,
    /// Wave definitions for the level.
    #[serde(default)]
    pub waves: Vec<WaveDef>,
}

#[derive(Component)]
pub struct Wall {}

#[derive(Component)]
pub struct AlienGoal;

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
