use bevy::app::{App, PluginGroup};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use avian3d::prelude::PhysicsPlugins;
use bevy_skein::SkeinPlugin;
use bevy_wind_waker_shader::flat::FlatShaderPlugin;
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use camera::components::CameraOffset;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use control::components::CharacterControl;
use crate::game_state::game_state_plugin::GamePlugin;

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;
pub(crate) mod alien;
pub(crate) mod ai;
pub(crate) mod towers;
pub(crate) mod ui;
mod control;
mod building;
mod map;
pub(crate) mod game_state;
mod animation;
mod constants;
mod assets;
pub(crate) mod settings;
pub(crate) mod model_settings;
pub(crate) mod poly_pizza;
pub(crate) mod character_creator;
pub(crate) mod sprite_billboard;
pub(crate) mod asset_browser;
pub(crate) mod player_setup;
pub(crate) mod map_editor;
pub mod behavior;

fn create_map(seed: Option<u64>, width: usize, height: usize, output: Option<String>) {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42)
    });

    let output = output.unwrap_or_else(|| {
        let dir = std::path::Path::new("assets/maps");
        let n = (1u32..)
            .find(|n| !dir.join(format!("map_{n}.ron")).exists())
            .unwrap_or(1);
        dir.join(format!("map_{n}.ron"))
            .to_string_lossy()
            .into_owned()
    });

    let map = crate::map::map_generator::generate_suburb_map(seed, width, height);

    let pretty = ron::ser::PrettyConfig::new().depth_limit(4);
    let out = ron::ser::to_string_pretty(&map, pretty)
        .unwrap_or_else(|e| panic!("Serialization failed: {e}"));
    std::fs::write(&output, out)
        .unwrap_or_else(|e| panic!("Cannot write {output}: {e}"));
    println!("Created {output}  (seed={seed} width={width} height={height})");
}

fn parse_create_map_args(args: &[String]) -> Option<()> {
    if !args.iter().any(|a| a == "--create-map") {
        return None;
    }
    let mut seed: Option<u64> = None;
    let mut width: usize = 32;
    let mut height: usize = 12;
    let mut output: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed"   => { seed   = args.get(i + 1).and_then(|v| v.parse().ok()); i += 2; }
            "--w"      => { width  = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(width); i += 2; }
            "--h"      => { height = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(height); i += 2; }
            "--output" => { output = args.get(i + 1).cloned(); i += 2; }
            _ => { i += 1; }
        }
    }
    create_map(seed, width, height, output);
    Some(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if parse_create_map_args(&args).is_some() {
        return;
    }

    App::new()
        .register_type::<CameraOffset>()
        .register_type::<CurrentTile>()
        .register_type::<CharacterControl>()
        .register_type::<Health>()
        .register_type::<AvoidWallsData>()
        .register_type::<ApproachAndAttackPlayerData>()
        .add_plugins(
            DefaultPlugins.set(
                LogPlugin {
                    filter: "wgpu_core=warn,wgpu_hal=warn".into(),
                    level: log::Level::INFO,
                    ..Default::default()
                }))
      .add_plugins(SkeinPlugin::default())
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(FlatShaderPlugin::global())
        .add_plugins(GamePlugin)
        .run();
}
