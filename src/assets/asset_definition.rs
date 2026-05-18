use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_scale() -> f32 { 1.0 }

/// Persisted definition for one imported asset. Written to `assets/defs/*.ron`
/// by the asset browser and read at runtime to drive hidden-node lists and
/// animation mappings without hard-coding them in source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDefinition {
    pub model_path: String,
    /// Uniform scale factor applied to the model so it has a meaningful real-world size.
    /// Computed as `target_height_m / mesh_aabb_height` in the asset browser.
    #[serde(default = "default_scale")]
    pub scale: f32,
    /// Node names that should be hidden when this model is used in-game.
    #[serde(default)]
    pub hidden_nodes: Vec<String>,
    /// Maps game-state keys (e.g. "idle", "walk", "throwing") to GLB clip
    /// name fragments.  Matched the same way as `AnimMapping` in ModelSettings.
    #[serde(default)]
    pub animation_mapping: HashMap<String, String>,
}

impl Default for AssetDefinition {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            scale: 1.0,
            hidden_nodes: Vec::new(),
            animation_mapping: HashMap::new(),
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
