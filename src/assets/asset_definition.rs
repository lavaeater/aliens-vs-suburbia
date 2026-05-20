use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_scale() -> f32 { 1.0 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyProps {
    pub health: f32,
    pub speed: f32,
    pub coin_drop: u32,
}

impl Default for EnemyProps {
    fn default() -> Self { Self { health: 100.0, speed: 2.0, coin_drop: 5 } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TowerProps {
    pub health: f32,
    pub cost: u32,
    pub range: f32,
    pub damage: f32,
    pub fire_rate_per_minute: f32,
}

impl Default for TowerProps {
    fn default() -> Self { Self { health: 200.0, cost: 50, range: 4.0, damage: 20.0, fire_rate_per_minute: 30.0 } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainProps {
    pub blocks_enemies: bool,
    pub blocks_players: bool,
    /// None = indestructible.
    pub health: Option<f32>,
}

impl Default for TerrainProps {
    fn default() -> Self { Self { blocks_enemies: true, blocks_players: false, health: None } }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum ItemKind {
    #[default]
    Decorative,
    HealthPickup { amount: f32 },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemProps {
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum PlayerAbility {
    #[default]
    Bombardment,
    Healing,
    Whirlwind,
    GoldDigger,
}

fn default_throw_rate() -> f32 { 60.0 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerProps {
    #[serde(default)]
    pub ability: PlayerAbility,
    /// Balls thrown per minute. Defaults to 60 (one per second).
    #[serde(default = "default_throw_rate")]
    pub throw_rate_per_minute: f32,
}

impl Default for PlayerProps {
    fn default() -> Self {
        Self { ability: PlayerAbility::default(), throw_rate_per_minute: default_throw_rate() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelType {
    Player(PlayerProps),
    Tower(TowerProps),
    Terrain(TerrainProps),
    Item(ItemProps),
    Enemy(EnemyProps),
}

impl Default for ModelType {
    fn default() -> Self { ModelType::Player(PlayerProps::default()) }
}

impl ModelType {
    pub fn label(&self) -> &'static str {
        match self {
            ModelType::Player(_)   => "Player",
            ModelType::Tower(_)    => "Tower",
            ModelType::Terrain(_)  => "Terrain",
            ModelType::Item(_)     => "Item",
            ModelType::Enemy(_)    => "Enemy",
        }
    }

    pub fn all_labels() -> &'static [&'static str] {
        &["Player", "Tower", "Terrain", "Item", "Enemy"]
    }

    /// Return a default instance for each label.
    pub fn from_label(label: &str) -> Self {
        match label {
            "Tower"   => ModelType::Tower(TowerProps::default()),
            "Terrain" => ModelType::Terrain(TerrainProps::default()),
            "Item"    => ModelType::Item(ItemProps::default()),
            "Enemy"   => ModelType::Enemy(EnemyProps::default()),
            _         => ModelType::Player(PlayerProps::default()),
        }
    }
}

/// Persisted definition for one imported asset. Written to `assets/defs/*.ron`
/// by the asset browser and read at runtime to drive hidden-node lists and
/// animation mappings without hard-coding them in source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDefinition {
    pub model_path: String,
    /// Uniform scale factor applied to the model so it has a meaningful real-world size.
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)]
    pub model_type: ModelType,
    /// Node names that should be hidden when this model is used in-game.
    #[serde(default)]
    pub hidden_nodes: Vec<String>,
    /// Maps game-state keys (e.g. "idle", "walk", "throwing") to clip name fragments.
    /// Values may be plain fragments ("idle") searched within the model's own GLTF,
    /// or "SourceStem|ClipFragment" to pull from an external file listed in animation_sources.
    #[serde(default)]
    pub animation_mapping: HashMap<String, String>,
    /// Paths of external GLB/GLTF files that supply additional animation clips.
    /// Same convention as model_path: relative to the assets/ folder, no "assets/" prefix.
    /// e.g. "packs/AnimPack.glb"
    #[serde(default)]
    pub animation_sources: Vec<String>,
}

impl Default for AssetDefinition {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            scale: 1.0,
            model_type: ModelType::default(),
            hidden_nodes: Vec::new(),
            animation_mapping: HashMap::new(),
            animation_sources: Vec::new(),
        }
    }
}

impl AssetDefinition {
    pub fn def_path(model_path: &str) -> std::path::PathBuf {
        let stem = std::path::Path::new(model_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("model");
        std::path::PathBuf::from("assets/defs").join(format!("{stem}.ron"))
    }

    /// Load from `assets/defs/<stem>.ron`. Returns `None` if no file exists.
    pub fn load(model_path: &str) -> Option<Self> {
        let path = Self::def_path(model_path);
        let text = std::fs::read_to_string(&path).ok()?;
        ron::from_str(&text).ok()
    }

    pub fn save(&self) {
        let path = Self::def_path(&self.model_path);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(path, text);
        }
    }
}
