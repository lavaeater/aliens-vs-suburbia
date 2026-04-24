use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;
use crate::settings::resources::GameSettings;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameSettings::load())
            .add_systems(Update, settings_keyboard_system);
    }
}

/// Keyboard shortcuts for tweaking settings at runtime:
///   F2          — save settings to disk
///   Z / X       — decrease / increase zoom
///   C / V       — decrease / increase camera pitch
///   N / M       — rotate camera left / right (yaw)
///   P           — toggle Orthographic ↔ Perspective
fn settings_keyboard_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<GameSettings>,
) {
    use crate::settings::resources::ProjectionMode;

    let mut changed = false;

    if keys.just_pressed(KeyCode::KeyP) {
        settings.projection = match settings.projection {
            ProjectionMode::Orthographic => ProjectionMode::Perspective,
            ProjectionMode::Perspective => ProjectionMode::Orthographic,
        };
        changed = true;
    }

    if keys.just_pressed(KeyCode::KeyZ) {
        settings.zoom = (settings.zoom - 1.0).max(1.0);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyX) {
        settings.zoom = (settings.zoom + 1.0).min(60.0);
        changed = true;
    }

    if keys.just_pressed(KeyCode::KeyC) {
        settings.pitch_degrees = (settings.pitch_degrees - 5.0).max(-89.0);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        settings.pitch_degrees = (settings.pitch_degrees + 5.0).min(-5.0);
        changed = true;
    }

    if keys.just_pressed(KeyCode::KeyN) {
        settings.yaw_degrees = (settings.yaw_degrees - 15.0).rem_euclid(360.0);
        changed = true;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        settings.yaw_degrees = (settings.yaw_degrees + 15.0).rem_euclid(360.0);
        changed = true;
    }

    if keys.just_pressed(KeyCode::F2) || changed {
        settings.save();
    }
}
