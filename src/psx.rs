//! `--psx`: PlayStation 1 style rendering from `bevy_psx` (our fork, branch `bevy-0.20`),
//! tried as an alternative to the retro look (`src/retro/`), which it replaces for the run.
//!
//! The game camera renders at a low resolution (320 x 240, widened to the window's aspect)
//! and is upscaled with nearest filtering; every `StandardMaterial` is turned into the
//! PSX material (vertex snapping, the wobble of the PS1's integer geometry). `bevy_psx` is
//! global once added: it spawns its own upscale camera and converts every material, so it
//! is only added with the flag.
//!
//! Known gaps, as a test: mouse aim maps the cursor through the full-size viewport, not
//! the low-res image (the retro look's `RetroView` does that mapping); the editors are not
//! meant to run under it.

use bevy::prelude::*;
use bevy_psx::prelude::{PsxCamera, PsxCameraPlugin, PsxSettings};

use crate::camera::components::GameCamera;

pub struct PsxLookPlugin;

impl Plugin for PsxLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PsxCameraPlugin)
            // Its default maps every colour to a small built-in palette, which turns a
            // night level black and red. Keep what makes it PS1: wobbling vertices, 15-bit
            // style colour steps, a light ordered dither.
            .insert_resource(PsxSettings {
                use_palette: false,
                quantize_enabled: true,
                quantize_steps: 32,
                dither_strength: 0.05,
                ..PsxSettings::default()
            })
            .add_systems(Update, (mark_game_camera, upscale_under_the_ui));
    }
}

/// `bevy_psx`'s upscale camera (the `Camera2d` drawing the low-res image on the window) and
/// our UI camera both come with order 1, which leaves which one draws last undefined. Put
/// the upscale camera at 0, under the HUD. It is the one 2D camera without our markers.
fn upscale_under_the_ui(
    mut cameras: Query<
        &mut Camera,
        (
            Added<Camera2d>,
            Without<IsDefaultUiCamera>,
            Without<crate::ui::spawn_ui::StateMarker>,
        ),
    >,
) {
    for mut camera in &mut cameras {
        if camera.order == 1 {
            camera.order = 0;
        }
    }
}

/// Every game camera renders through the PSX pipeline.
fn mark_game_camera(
    mut commands: Commands,
    cameras: Query<Entity, (With<GameCamera>, Without<PsxCamera>)>,
) {
    for camera in &cameras {
        commands.entity(camera).insert(PsxCamera);
    }
}
