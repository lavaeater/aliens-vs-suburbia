use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use crate::animation::animation_plugin::AnimationKey;

pub const MODEL_SETTINGS_PATH: &str = "player-settings.ron";
pub const DEFAULT_CHARACTER_FOLDER: &str = "packs/toon-shooter/characters";

/// Per-animation-key clip name override.  If empty the default search
/// fragment from `AnimationKey::default_search()` is used.  Names are
/// matched against GLTF clip names via `clip_matches()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnimMapping {
    #[serde(default)] pub idle: String,
    #[serde(default)] pub idle_shoot: String,
    #[serde(default)] pub walk: String,
    #[serde(default)] pub walk_shoot: String,
    #[serde(default)] pub run: String,
    #[serde(default)] pub run_shoot: String,
    #[serde(default)] pub run_gun: String,
    #[serde(default)] pub duck: String,
    #[serde(default)] pub jump: String,
    #[serde(default)] pub jump_idle: String,
    #[serde(default)] pub jump_land: String,
    #[serde(default)] pub punch: String,
    #[serde(default)] pub wave: String,
    #[serde(default)] pub yes: String,
    #[serde(default)] pub no: String,
    #[serde(default)] pub death: String,
    #[serde(default)] pub hit_react: String,
    #[serde(default)] pub throwing: String,
    #[serde(default)] pub building: String,
}

impl AnimMapping {
    pub fn get(&self, key: AnimationKey) -> &str {
        match key {
            AnimationKey::Idle      => &self.idle,
            AnimationKey::IdleShoot => &self.idle_shoot,
            AnimationKey::Walk      => &self.walk,
            AnimationKey::WalkShoot => &self.walk_shoot,
            AnimationKey::Run       => &self.run,
            AnimationKey::RunShoot  => &self.run_shoot,
            AnimationKey::RunGun    => &self.run_gun,
            AnimationKey::Duck      => &self.duck,
            AnimationKey::Jump      => &self.jump,
            AnimationKey::JumpIdle  => &self.jump_idle,
            AnimationKey::JumpLand  => &self.jump_land,
            AnimationKey::Punch     => &self.punch,
            AnimationKey::Wave      => &self.wave,
            AnimationKey::Yes       => &self.yes,
            AnimationKey::No        => &self.no,
            AnimationKey::Death     => &self.death,
            AnimationKey::HitReact  => &self.hit_react,
            AnimationKey::Throwing  => &self.throwing,
            AnimationKey::Building  => &self.building,
        }
    }

    pub fn set(&mut self, key: AnimationKey, name: String) {
        match key {
            AnimationKey::Idle      => self.idle       = name,
            AnimationKey::IdleShoot => self.idle_shoot = name,
            AnimationKey::Walk      => self.walk       = name,
            AnimationKey::WalkShoot => self.walk_shoot = name,
            AnimationKey::Run       => self.run        = name,
            AnimationKey::RunShoot  => self.run_shoot  = name,
            AnimationKey::RunGun    => self.run_gun    = name,
            AnimationKey::Duck      => self.duck       = name,
            AnimationKey::Jump      => self.jump       = name,
            AnimationKey::JumpIdle  => self.jump_idle  = name,
            AnimationKey::JumpLand  => self.jump_land  = name,
            AnimationKey::Punch     => self.punch      = name,
            AnimationKey::Wave      => self.wave       = name,
            AnimationKey::Yes       => self.yes        = name,
            AnimationKey::No        => self.no         = name,
            AnimationKey::Death     => self.death      = name,
            AnimationKey::HitReact  => self.hit_react  = name,
            AnimationKey::Throwing  => self.throwing   = name,
            AnimationKey::Building  => self.building   = name,
        }
    }
}

/// All .glb/.gltf filenames found in the active character folder, sorted.
/// Populated at startup and refreshed when `ModelSettings::character_folder` changes.
#[derive(Resource, Default)]
pub struct CharacterFolder {
    pub files: Vec<String>,
}

impl CharacterFolder {
    /// Bare name without extension, for display ("Character Soldier.glb" → "Character Soldier").
    pub fn display_name(file: &str) -> &str {
        file.strip_suffix(".gltf")
            .or_else(|| file.strip_suffix(".glb"))
            .unwrap_or(file)
    }
}

/// Scans `assets/{folder}` and returns all model filenames, sorted.
pub fn scan_character_folder(folder: &str) -> Vec<String> {
    let dir = std::path::Path::new("assets").join(folder);
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_str()?;
            if ext == "glb" || ext == "gltf" {
                p.file_name()?.to_str().map(str::to_string)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

/// Animation clip names available in the currently loaded player GLTF, sorted.
/// Used by the F2 panel to cycle through available clips for each mapping slot.
#[derive(Resource, Default)]
pub struct PlayerAnimClips {
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
pub struct ModelSettings {
    /// Folder containing character models, relative to `assets/`.
    /// All models in this folder share the transform/animation settings below.
    #[serde(default = "default_character_folder")]
    pub character_folder: String,
    /// Index into the sorted file list within `character_folder`.
    #[serde(default)]
    pub character_index: usize,
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)] pub translation_x: f32,
    #[serde(default)] pub translation_y: f32,
    #[serde(default)] pub translation_z: f32,
    #[serde(default)] pub rotation_y_degrees: f32,
    #[serde(default)] pub anim_mapping: AnimMapping,
}

fn default_character_folder() -> String { DEFAULT_CHARACTER_FOLDER.to_string() }
fn default_scale() -> f32 { 1.0 }

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            character_folder: default_character_folder(),
            character_index: 0,
            scale: default_scale(),
            translation_x: 0.0,
            translation_y: 0.0,
            translation_z: 0.0,
            rotation_y_degrees: 0.0,
            anim_mapping: AnimMapping::default(),
        }
    }
}

impl ModelSettings {
    /// Full asset-relative path for the currently selected character.
    pub fn current_model_path(&self, folder: &CharacterFolder) -> Option<String> {
        folder.files.get(self.character_index)
            .map(|f| format!("{}/{f}", self.character_folder))
    }

    pub fn load() -> Self {
        if let Ok(text) = std::fs::read_to_string(MODEL_SETTINGS_PATH)
            && let Ok(s) = ron::from_str::<ModelSettings>(&text)
        {
            return s;
        }
        ModelSettings::default()
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(MODEL_SETTINGS_PATH, text);
        }
    }
}
