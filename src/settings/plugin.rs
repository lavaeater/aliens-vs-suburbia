use bevy::app::{App, Plugin, Update};
use bevy::prelude::*;
use crate::game_state::GameState;
use crate::settings::resources::GameSettings;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameSettings::load())
            .add_systems(
                Update,
                settings_keyboard_system
                    .run_if(in_state(GameState::InGame).or(in_state(GameState::ModelShowcase))),
            );
    }
}

/// Keyboard shortcuts for tweaking settings at runtime:
///   F2          — save settings to disk
///   Z / X       — decrease / increase zoom
///   C / V       — decrease / increase camera pitch
///   N / M       — rotate camera left / right (yaw)
///   P           — toggle Orthographic ↔ Perspective
///   [ / ]       — decrease / increase ortho viewport height
///   F3 / F4     — decrease / increase near clip (0.05 steps for persp, 50 for ortho)
///   F5 / F6     — decrease / increase far clip (100 steps)
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

    if keys.just_pressed(KeyCode::BracketLeft) {
        settings.ortho_viewport_height = (settings.ortho_viewport_height - 0.25).max(0.25);
        changed = true;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        settings.ortho_viewport_height += 0.25;
        changed = true;
    }

    if keys.just_pressed(KeyCode::F3) {
        match settings.projection {
            ProjectionMode::Perspective => {
                settings.persp_near = (settings.persp_near - 0.05).max(0.01);
            }
            ProjectionMode::Orthographic => {
                settings.ortho_near -= 50.0;
            }
        }
        changed = true;
    }
    if keys.just_pressed(KeyCode::F4) {
        match settings.projection {
            ProjectionMode::Perspective => {
                settings.persp_near = (settings.persp_near + 0.05).min(settings.persp_far - 0.1);
            }
            ProjectionMode::Orthographic => {
                settings.ortho_near = (settings.ortho_near + 50.0).min(0.0);
            }
        }
        changed = true;
    }

    if keys.just_pressed(KeyCode::F5) {
        match settings.projection {
            ProjectionMode::Perspective => {
                settings.persp_far = (settings.persp_far - 100.0).max(settings.persp_near + 1.0);
            }
            ProjectionMode::Orthographic => {
                settings.ortho_far = (settings.ortho_far - 100.0).max(1.0);
            }
        }
        changed = true;
    }
    if keys.just_pressed(KeyCode::F6) {
        match settings.projection {
            ProjectionMode::Perspective => settings.persp_far += 100.0,
            ProjectionMode::Orthographic => settings.ortho_far += 100.0,
        }
        changed = true;
    }

    if keys.just_pressed(KeyCode::F2) || changed {
        settings.save();
    }
}
