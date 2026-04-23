use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub const SETTINGS_PATH: &str = "game-settings.ron";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProjectionMode {
    Orthographic,
    Perspective,
}

#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
pub struct GameSettings {
    #[serde(default = "default_projection")]
    pub projection: ProjectionMode,
    /// Orthographic scale / perspective FOV in degrees
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    /// Camera pitch in degrees (negative = looking down)
    #[serde(default = "default_pitch")]
    pub pitch_degrees: f32,
    /// Camera yaw in degrees — rotates the orbit angle around the player
    #[serde(default = "default_yaw")]
    pub yaw_degrees: f32,
    #[serde(default = "default_speed")]
    pub player_speed_multiplier: f32,
}

fn default_projection() -> ProjectionMode { ProjectionMode::Orthographic }
fn default_zoom() -> f32 { 8.0 }
fn default_pitch() -> f32 { -45.0 }
fn default_yaw() -> f32 { 45.0 }
fn default_speed() -> f32 { 1.0 }

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            projection: default_projection(),
            zoom: default_zoom(),
            pitch_degrees: default_pitch(),
            yaw_degrees: default_yaw(),
            player_speed_multiplier: default_speed(),
        }
    }
}

impl GameSettings {
    pub fn load() -> Self {
        let path = std::path::Path::new(SETTINGS_PATH);
        if path.exists() {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(settings) = ron::from_str::<GameSettings>(&text) {
                    return settings;
                }
            }
        }
        GameSettings::default()
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(SETTINGS_PATH, text);
        }
    }
}
