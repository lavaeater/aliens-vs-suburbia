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
    /// One "player unit" (p) in world space. Set to the player character's visual height.
    /// All decoration scales in the map are expressed as multiples of this value.
    /// E.g. trees scaled 3–4p, bushes 0.5–1p, clutter 0.1–0.25p.
    #[serde(default = "default_player_unit")]
    pub player_unit: f32,
    // Orthographic-specific
    #[serde(default = "default_ortho_near")]
    pub ortho_near: f32,
    #[serde(default = "default_ortho_far")]
    pub ortho_far: f32,
    /// Base vertical world units visible at ortho scale = 1. Keyboard: [ / ]
    #[serde(default = "default_ortho_viewport_height")]
    pub ortho_viewport_height: f32,
    // Perspective-specific
    #[serde(default = "default_persp_fov")]
    pub persp_fov: f32,
    #[serde(default = "default_persp_near")]
    pub persp_near: f32,
    #[serde(default = "default_persp_far")]
    pub persp_far: f32,
}

fn default_projection() -> ProjectionMode { ProjectionMode::Orthographic }
fn default_zoom() -> f32 { 8.0 }
fn default_pitch() -> f32 { -45.0 }
fn default_yaw() -> f32 { 45.0 }
fn default_speed() -> f32 { 1.0 }
fn default_player_unit() -> f32 { 1.0 }
fn default_ortho_near() -> f32 { -1000.0 }
fn default_ortho_far() -> f32 { 1000.0 }
fn default_ortho_viewport_height() -> f32 { 2.0 }
fn default_persp_fov()  -> f32 { 60.0 }
fn default_persp_near() -> f32 { 0.1 }
fn default_persp_far() -> f32 { 1000.0 }

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            projection: default_projection(),
            zoom: default_zoom(),
            pitch_degrees: default_pitch(),
            yaw_degrees: default_yaw(),
            player_speed_multiplier: default_speed(),
            player_unit: default_player_unit(),
            ortho_near: default_ortho_near(),
            ortho_far: default_ortho_far(),
            ortho_viewport_height: default_ortho_viewport_height(),
            persp_fov: default_persp_fov(),
            persp_near: default_persp_near(),
            persp_far: default_persp_far(),
        }
    }
}

impl GameSettings {
    pub fn load() -> Self {
        let path = std::path::Path::new(SETTINGS_PATH);
        if path.exists()
            && let Ok(text) = std::fs::read_to_string(path)
                && let Ok(settings) = ron::from_str::<GameSettings>(&text) {
                    return settings;
                }
        GameSettings::default()
    }

    pub fn save(&self) {
        if let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = std::fs::write(SETTINGS_PATH, text);
        }
    }
}
