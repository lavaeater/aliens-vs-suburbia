use crate::physics::prelude::{CollisionLayers, LayerMask, RigidBody};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A user-placed model (from an AssetDefinition) on a specific grid tile.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct TilePlacement {
    pub x: i32,
    pub y: i32,
    /// Path to the `.ron` def file, relative to project root.
    pub def_path: String,
    /// Rotation in 45° steps (0–7).
    #[serde(default)]
    pub rotation_steps: u8,
}

/// A house from `assets/houses/<house>.ron` standing in a map. The game stamps it into
/// the tiles when the level loads (`map::house::bake_houses`); the map editor keeps it
/// whole, so it can be moved or removed.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct HousePlacement {
    /// The house's name: its file stem under `assets/houses`.
    pub house: String,
    /// The top-left tile of the house's footprint.
    pub x: i32,
    pub y: i32,
    /// Clockwise quarter turns, 0-3.
    #[serde(default)]
    pub turns: u8,
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
        Self {
            enemy_def: String::new(),
            count: 10,
            spawn_rate_per_minute: 20.0,
        }
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

/// Random loot bags scattered over the open floor when the map loads
/// (`loot::bags`): `min..=max` bags for every `per_tiles` open tiles, each rolling
/// `table` when opened. Written `loot: 1-5 per 20` in a `.map`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LootSpread {
    pub min: u32,
    pub max: u32,
    pub per_tiles: u32,
    /// The loot table each bag rolls (`assets/loot/<table>.ron`).
    #[serde(default = "default_loot_table")]
    pub table: String,
    /// The defs a bag is drawn with, one picked at random per bag (Item defs; their own
    /// kind is ignored).
    #[serde(default = "default_loot_bags")]
    pub bags: Vec<String>,
}

pub fn default_loot_table() -> String {
    crate::loot::DEFAULT_BAG_TABLE.into()
}

pub fn default_loot_bags() -> Vec<String> {
    vec!["defs/Trash Bag.ron".into(), "defs/Trash Bags.ron".into()]
}

impl Default for LootSpread {
    fn default() -> Self {
        Self {
            min: 1,
            max: 3,
            per_tiles: 50,
            table: default_loot_table(),
            bags: default_loot_bags(),
        }
    }
}

const fn default_scale() -> f32 {
    1.0
}

const fn default_map_width() -> usize {
    14
}
const fn default_map_height() -> usize {
    24
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MapFile {
    /// Display name. Optional; the `.map` text format writes it under `[properties]`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
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
    /// Houses placed in the map (`assets/houses`), stamped into `tiles` on load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub houses: Vec<HousePlacement>,
    /// Wave definitions for the level.
    #[serde(default)]
    pub waves: Vec<WaveDef>,
    /// The level's own turbofacts stories (a RON list of `Story`), e.g.
    /// `"campaigns/main/level_01/level_01_story.ron"` (next to the map; `assets::content`).
    /// Loaded on top of the base level-flow stories when the map loads. Empty = none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stories: String,
    /// Random loot bags on the floor. `None` = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<LootSpread>,
    /// How many aliens may reach the goal before the level is lost. `None` = the default
    /// (`LevelTracker`'s 10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape_limit: Option<u32>,
    /// Weapon defs every player carries on this level instead of their own (the first is
    /// held). Empty = each character's own loadout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapons: Vec<String>,
    /// The team starts the level with at least this many coins (for towers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coins: Option<u32>,
}

impl MapFile {
    /// Every path the map names (placements, wave enemies, weapons, loot bags, stories)
    /// in the short content form, `"defs/Bunny.ron"` (`assets::paths`). Done on every
    /// read and write, so an older map with `assets/` prefixes reads the same and
    /// saves without them.
    pub fn normalize_paths(&mut self) {
        use crate::assets::paths::normalize;
        self.placements
            .iter_mut()
            .for_each(|p| normalize(&mut p.def_path));
        self.waves
            .iter_mut()
            .for_each(|w| normalize(&mut w.enemy_def));
        self.weapons.iter_mut().for_each(normalize);
        if let Some(loot) = &mut self.loot {
            loot.bags.iter_mut().for_each(normalize);
        }
        normalize(&mut self.stories);
    }
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

#[derive(Hash, PartialEq, Eq, Clone, Reflect, Component, Default)]
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
    pub fn builtin(
        name: &str,
        model_key: &'static str,
        file: &str,
        cost: u32,
        tower: Option<crate::assets::asset_definition::TowerProps>,
    ) -> Self {
        Self {
            name: name.to_string(),
            file: file.to_string(),
            cost,
            model_key: Some(model_key),
            scale: 1.0,
            tower,
        }
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
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(def) = ron::from_str::<AssetDefinition>(&text) else {
                    continue;
                };
                let ModelType::Tower(props) = &def.model_type else {
                    continue;
                };
                let name = path
                    .file_stem()
                    .map_or_default(|s| s.to_string_lossy().into_owned());
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
    pub tile: (usize, usize),
}

#[derive(Component, Debug, Reflect, Default)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct AlienSpawnPoint {
    pub spawn_rate_per_minute: f32,
    pub spawn_cool_down: f32,
}

impl AlienSpawnPoint {
    pub const fn new(spawn_rate_per_minute: f32) -> Self {
        Self {
            spawn_rate_per_minute,
            spawn_cool_down: 0.0,
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
