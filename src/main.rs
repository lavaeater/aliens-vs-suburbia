use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::game_state::game_state_plugin::GamePlugin;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use avian3d::prelude::{PhysicsDebugPlugin, PhysicsGizmos, PhysicsPlugins};
use bevy::app::{App, PluginGroup};
use bevy::gizmos::AppGizmoBuilder;
use bevy::gizmos::config::GizmoConfig;
use bevy::log::LogPlugin;
use bevy::{DefaultPlugins, log};
use bevy_skein::SkeinPlugin;
use bevy_wind_waker_shader::flat::FlatShaderPlugin;
use camera::components::CameraOffset;
use control::components::CharacterControl;

pub(crate) mod ai;
pub(crate) mod alien;
mod animation;
pub(crate) mod asset_browser;
mod assets;
pub mod behavior;
mod building;
pub(crate) mod camera;
mod constants;
mod control;
pub(crate) mod facts;
pub(crate) mod game_state;
pub(crate) mod general;
pub(crate) mod gore;
pub(crate) mod house_editor;
mod map;
pub(crate) mod map_editor;
#[cfg(feature = "map-editor")]
pub(crate) mod map_editor_tui;
pub(crate) mod model_settings;
pub(crate) mod music;
pub(crate) mod player;
pub(crate) mod player_setup;
pub(crate) mod playground;
pub(crate) mod poly_pizza;
pub(crate) mod settings;
pub(crate) mod towers;
pub(crate) mod ui;

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

    let map = map::map_generator::generate_suburb_map(seed, width, height);

    let pretty = ron::ser::PrettyConfig::new().depth_limit(4);
    let out = ron::ser::to_string_pretty(&map, pretty)
        .unwrap_or_else(|e| panic!("Serialization failed: {e}"));
    std::fs::write(&output, out).unwrap_or_else(|e| panic!("Cannot write {output}: {e}"));
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
            "--seed" => {
                seed = args.get(i + 1).and_then(|v| v.parse().ok());
                i += 2;
            }
            "--w" => {
                width = args
                    .get(i + 1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(width);
                i += 2;
            }
            "--h" => {
                height = args
                    .get(i + 1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(height);
                i += 2;
            }
            "--output" => {
                output = args.get(i + 1).cloned();
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }
    create_map(seed, width, height, output);
    Some(())
}

fn print_help() {
    println!("Aliens vs Suburbia\n");
    println!("USAGE:");
    println!("  cargo run                                  Launch the game");
    println!("  cargo run -- --playground                  Launch straight into the playground");
    println!("  cargo run -- --asset-browser               Launch straight into the asset browser");
    println!("  cargo run --features map-editor -- --map-editor [--file <path>]");
    println!("                                             Launch the map editor TUI");
    println!("  cargo run -- --create-map [OPTIONS]        Generate a map file\n");
    println!("MAP EDITOR OPTIONS:");
    println!("  --file <path>    Open an existing map file (e.g. assets/maps/map_1.ron)\n");
    println!("CREATE MAP OPTIONS:");
    println!("  --seed <u64>     Random seed (default: time-based)");
    println!("  --w <usize>      Map width in tiles (default: 32)");
    println!("  --h <usize>      Map height in tiles (default: 12)");
    println!("  --output <path>  Output file path (default: assets/maps/map_N.ron)");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    if parse_create_map_args(&args).is_some() {
        return;
    }

    #[cfg(feature = "map-editor")]
    if args.iter().any(|a| a == "--map-editor") {
        let file = args.iter().skip_while(|a| *a != "--file").nth(1).cloned();
        map_editor_tui::run(file).unwrap();
        return;
    }

    // Boot straight into the playground, skipping the menu. Handy when iterating on the
    // playground itself; the menu button does the same two things.
    let straight_to_playground = args.iter().any(|a| a == "--playground");
    // Same idea for the asset browser: it is a tool screen you open, tweak in, and reload.
    let straight_to_asset_browser = args.iter().any(|a| a == "--asset-browser");

    let mut app = App::new();
    app.register_type::<CameraOffset>()
        .register_type::<CurrentTile>()
        .register_type::<CharacterControl>()
        .register_type::<Health>()
        .register_type::<AvoidWallsData>()
        .register_type::<ApproachAndAttackPlayerData>()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            filter: "wgpu_core=warn,wgpu_hal=warn".into(),
            level: log::Level::INFO,
            ..Default::default()
        }))
        .add_plugins(SkeinPlugin::default())
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(PhysicsDebugPlugin)
        .insert_gizmo_config(
            PhysicsGizmos::default(),
            GizmoConfig {
                enabled: false,
                ..Default::default()
            },
        )
        .add_plugins(FlatShaderPlugin::global())
        // .add_plugins(PixelShaderPlugin::default())
        .add_plugins(GamePlugin);

    if straight_to_asset_browser {
        app.add_systems(bevy::app::Startup, |mut next: bevy::prelude::ResMut<bevy::prelude::NextState<game_state::GameState>>| {
            next.set(game_state::GameState::AssetBrowser);
        });
    }

    if straight_to_playground {
        app.add_systems(bevy::app::Startup, |mut commands: bevy::prelude::Commands,
                                             mut next: bevy::prelude::ResMut<bevy::prelude::NextState<game_state::GameState>>| {
            commands.init_resource::<playground::state::PlaygroundSession>();
            next.set(game_state::GameState::InGame);
        });
    }

    app.run();
}
