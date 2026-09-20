use bevy::prelude::*;
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
    /// Row-major grid of `BitFlags<MapFeatures>` values stored as u64.
    /// 0 = void (Nothing), non-zero = tile with feature flags set.
    #[serde(default)]
    pub tiles: Vec<Vec<u64>>,
    #[serde(default)]
    pub decorations: Vec<DecorationItem>,
    /// Items placed by the map editor (def-driven models on specific tiles).
    #[serde(default)]
    pub placements: Vec<TilePlacement>,
    /// Wave definitions for the level.
    #[serde(default)]
    pub waves: Vec<WaveDef>,
}

#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct Wall {}

#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct AlienGoal;

pub struct ModelDefinition {
    /// Display name and scene path. The build menu reads these through `BuildOption`;
    /// the table keeps them so the two stay side by side when edited.
    #[allow(dead_code)]
    pub name: &'static str,
    #[allow(dead_code)]
    pub file: &'static str,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub rigid_body: RigidBody,
    pub group: LayerMask,
    pub mask: LayerMask,
}

#[derive(Hash, PartialEq, Eq, Clone, Reflect,Component, Default)]
 #[type_path = "avs"]
pub struct Tower {}

impl ModelDefinition {
    pub fn create_collision_layers(&self) -> CollisionLayers {
        CollisionLayers::new(self.group, self.mask)
    }
}

#[derive(Resource)]
pub struct MapModelDefinitions {
    pub definitions: HashMap<&'static str, ModelDefinition>,
    /// What the build menu cycles through, in order.
    pub build_indicators: Vec<BuildOption>,
}

/// One entry in the build menu: either a built-in model from `definitions` or a
/// `Tower`-typed def file. Towers carry their props so placement can read kind and cost.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildOption {
    pub name: String,
    /// Scene asset path for the preview and the built thing.
    pub file: String,
    pub cost: u32,
    /// Built-in model definition key (collider size, layers). `None` = a def-driven tower
    /// using the standard tile-sized collider.
    pub model_key: Option<&'static str>,
    /// Model scale for def-driven entries (built-ins are authored at scale 1).
    pub scale: f32,
    pub tower: Option<crate::assets::asset_definition::TowerProps>,
}

impl BuildOption {
    pub fn builtin(name: &str, model_key: &'static str, file: &str, cost: u32, tower: Option<crate::assets::asset_definition::TowerProps>) -> Self {
        Self { name: name.to_string(), file: file.to_string(), cost, model_key: Some(model_key), scale: 1.0, tower }
    }

    /// Every `Tower`-typed def under `assets/defs`, sorted by name.
    pub fn scan_defs(dir: &str) -> Vec<Self> {
        use crate::assets::asset_definition::{AssetDefinition, ModelType};
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let Ok(def) = ron::from_str::<AssetDefinition>(&text) else { continue };
                let ModelType::Tower(props) = &def.model_type else { continue };
                let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                out.push(Self {
                    name,
                    file: format!("{}#Scene0", def.model_path),
                    cost: props.cost,
                    model_key: None,
                    scale: def.scale,
                    tower: Some(props.clone()),
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

#[derive(Component, Debug, Reflect, Default)]
pub struct CurrentTile {
    pub tile: (usize, usize)
}

#[derive(Component, Debug, Reflect, Default)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
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


#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct Floor {}
