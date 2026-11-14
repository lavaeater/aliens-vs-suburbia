//! `--plain`: the game without our own looks, for playtesting what is under them.
//!
//! Off: the retro look (palette, dither, pixels; `main.rs` passes `force_off`), the fog
//! and the valley fog banks (`FogLevel::Off`, unless `--fog` asks for some), the dread
//! colour grade and haze (`DespairSettings::max_strength` 0) and the scene outlines
//! (`auto_outline_scenes` skips while [`PlainLook`] exists). Kept: lighting, particles,
//! grass (`--no-grass` for that). Ctrl+K and Ctrl+O still turn retro and fog back on.

use bevy::prelude::*;

use crate::camera::fog::FogLevel;
use crate::gore::despair::DespairSettings;

/// Present in a `--plain` run.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PlainLook;

/// Add after `GamePlugin`, so its resources win over the defaults.
pub struct PlainLookPlugin;

impl Plugin for PlainLookPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlainLook)
            .insert_resource(FogLevel::Off)
            .insert_resource(DespairSettings {
                max_strength: 0.0,
                ..DespairSettings::default()
            });
    }
}
