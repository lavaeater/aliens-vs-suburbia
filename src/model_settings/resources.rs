use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use crate::animation::animation_plugin::AnimationKey;

pub const ANIM_KEYS: &[AnimationKey] = &[
    AnimationKey::Idle,
    AnimationKey::Walking,
    AnimationKey::Throwing,
    AnimationKey::Crawling,
    AnimationKey::Building,
];

#[derive(Resource, Default)]
pub struct DebugAnimSelection {
    pub index: usize,
    pub dirty: bool,
}

pub const MODEL_SETTINGS_PATH: &str = "assets/models/Adventurer-settings.ron";

#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
pub struct ModelSettings {
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)]
    pub translation_x: f32,
    #[serde(default)]
    pub translation_y: f32,
    #[serde(default)]
    pub translation_z: f32,
    #[serde(default)]
    pub rotation_y_degrees: f32,
}

fn default_scale() -> f32 { 1.0 }

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            scale: default_scale(),
            translation_x: 0.0,
            translation_y: 0.0,
            translation_z: 0.0,
            rotation_y_degrees: 0.0,
        }
    }
}

impl ModelSettings {
    pub fn load() -> Self {
        let path = std::path::Path::new(MODEL_SETTINGS_PATH);
        if path.exists() {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(settings) = ron::from_str::<ModelSettings>(&text) {
                    return settings;
                }
            }
        }
        ModelSettings::default()
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(MODEL_SETTINGS_PATH, text);
        }
    }
}
