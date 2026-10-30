// Proving `Sync` for bevy_render's `ViewTarget` walks deep into wgpu's types and passes
// the default limit of 128; newer nightlies flag that as `recursion_depth_exceeding_limit`
// (rust-lang/rust#159228), due to become a hard error. Compile-time only.
#![recursion_limit = "256"]

use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::game_state::game_state_plugin::GamePlugin;
use crate::general::components::Health;
use crate::general::components::map_components::CurrentTile;
use avian3d::prelude::{PhysicsDebugPlugin, PhysicsGizmos, PhysicsPlugins};
use bevy::gizmos::AppGizmoBuilder;
use bevy::gizmos::config::GizmoConfig;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::{DefaultPlugins, log};
use bevy_skein::SkeinPlugin;
use bevy_wind_waker_shader::flat::{FlatShaderBuilder, FlatShaderPlugin};
use bevy_wind_waker_shader::pixelate::PixelShaderPlugin;
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
pub(crate) mod campaign;
pub(crate) mod character_editor;
mod constants;
mod control;
pub(crate) mod crawl;
pub(crate) mod facts;
pub(crate) mod fullscreen;
pub(crate) mod fx;
pub(crate) mod game_state;
pub(crate) mod general;
pub(crate) mod gore;
pub(crate) mod house_editor;
pub(crate) mod items;
pub(crate) mod loot;
mod map;
pub(crate) mod map_editor;
#[cfg(feature = "map-editor")]
pub(crate) mod map_editor_tui;
pub(crate) mod model_settings;
pub(crate) mod music;
pub(crate) mod player;
pub(crate) mod player_setup;
pub(crate) mod poly_pizza;
pub(crate) mod settings;
pub mod skein;
pub(crate) mod towers;
pub(crate) mod ui;
pub(crate) mod validate;

fn create_map(
    seed: Option<u64>,
    width: usize,
    height: usize,
    output: Option<String>,
) -> Result<(), String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(42, |d| d.as_nanos() as u64)
    });

    let output = output.unwrap_or_else(|| {
        let dir = std::path::Path::new("assets/maps");
        // Bounded by the first filename that is free, and by u32::MAX regardless.
        #[allow(clippy::maybe_infinite_iter)]
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
        .map_err(|e| format!("Serialization failed: {e}"))?;
    std::fs::write(&output, out).map_err(|e| format!("Cannot write {output}: {e}"))?;
    println!("Created {output}  (seed={seed} width={width} height={height})");
    Ok(())
}

fn parse_create_map_args(args: &[String]) -> Option<Result<(), String>> {
    if !args.iter().any(|a| a == "--create-map") {
        return None;
    }
    let mut seed: Option<u64> = None;
    let mut width: usize = 32;
    let mut height: usize = 12;
    let mut output: Option<String> = None;

    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
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
    Some(create_map(seed, width, height, output))
}

/// `--convert-map <in> [<out>] [--force]`: RON <-> `.map` text, by extension. Without
/// `<out>` the file lands next to `<in>` with the other extension. Never overwrites an
/// existing file unless `--force` is given.
fn parse_convert_map_args(args: &[String]) -> Option<Result<(), String>> {
    let position = args.iter().position(|a| a == "--convert-map")?;
    let paths: Vec<&String> = args
        .iter()
        .skip(position.saturating_add(1))
        .take_while(|a| !a.starts_with("--"))
        .collect();
    let force = args.iter().any(|a| a == "--force");
    Some(convert_map(
        paths.first().copied(),
        paths.get(1).copied(),
        force,
    ))
}

fn convert_map(input: Option<&String>, output: Option<&String>, force: bool) -> Result<(), String> {
    use map::text_format;

    let input = std::path::Path::new(input.ok_or("--convert-map needs an input file")?);
    let output = output.map_or_else(
        || text_format::converted_path(input),
        std::path::PathBuf::from,
    );
    if output.exists() && !force {
        return Err(format!(
            "{} already exists; pass --force to overwrite it",
            output.display()
        ));
    }
    let map = text_format::read_map_file(input)?;
    text_format::write_map_file(&output, &map)?;
    println!("Converted {} -> {}", input.display(), output.display());
    Ok(())
}

fn print_help() {
    println!("Aliens vs Suburbia\n");
    println!("USAGE:");
    println!("  cargo run                                  Launch the game");
    println!(
        "  cargo run -- --editor                      Launch straight into the character editor"
    );
    println!("                                             (--playground still works as an alias)");
    println!("  cargo run -- --asset-browser               Launch straight into the asset browser");
    println!("  cargo run -- --house-editor                Launch straight into the house editor");
    println!("  cargo run -- --play [--level N]            Launch straight into a match (level 1, or N)");
    println!("  cargo run -- --fullscreen                  1920x1080 fullscreen (combines with the others)");
    println!("  cargo run --features map-editor -- --map-editor [--file <path>]");
    println!("                                             Launch the map editor TUI");
    println!("  cargo run -- --validate                    Check every content file and what it points at");
    println!("  cargo run -- --create-map [OPTIONS]        Generate a map file");
    println!("  cargo run -- --convert-map <in> [<out>] [--force]");
    println!(
        "                                             Convert a map between .ron and .map text"
    );
    println!(
        "                                             (out defaults to <in> with the other extension)\n"
    );
    println!("MAP EDITOR OPTIONS:");
    println!("  --file <path>    Open an existing map file (e.g. assets/maps/map_1.ron)\n");
    println!("CREATE MAP OPTIONS:");
    println!("  --seed <u64>     Random seed (default: time-based)");
    println!("  --w <usize>      Map width in tiles (default: 32)");
    println!("  --h <usize>      Map height in tiles (default: 12)");
    println!("  --output <path>  Output file path (default: assets/maps/map_N.ron)");
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return std::process::ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--validate") {
        return validate::run_cli();
    }

    if let Some(result) = parse_create_map_args(&args).or_else(|| parse_convert_map_args(&args)) {
        return match result {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    #[cfg(feature = "map-editor")]
    if args.iter().any(|a| a == "--map-editor") {
        let file = args.iter().skip_while(|a| *a != "--file").nth(1).cloned();
        return match map_editor_tui::run(file) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                std::process::ExitCode::FAILURE
            }
        };
    }

    // Boot straight into the character editor, skipping the menu. Handy when iterating on
    // the editor itself; the menu button does the same two things. `--playground` is the
    // editor's old name, kept so muscle memory still works.
    let straight_to_editor = args.iter().any(|a| a == "--editor" || a == "--playground");
    // Same idea for the asset browser: it is a tool screen you open, tweak in, and reload.
    let straight_to_asset_browser = args.iter().any(|a| a == "--asset-browser");
    let straight_to_house_editor = args.iter().any(|a| a == "--house-editor");
    // And into a match, skipping the menu and the player setup (one keyboard player, with a
    // connected pad handed to them by `assign_gamepads`' fallback).
    let straight_to_game = args.iter().any(|a| a == "--play");
    // `--play --level N` starts the campaign at level N (Start Game always begins at 1).
    let start_level: Option<usize> = args
        .iter()
        .skip_while(|a| *a != "--level")
        .nth(1)
        .and_then(|n| n.parse().ok());

    // 1920x1080 fullscreen on the primary monitor (`fullscreen.rs`).
    let fullscreen = args.iter().any(|a| a == "--fullscreen");

    let mut app = App::new();
    app.register_type::<CameraOffset>()
        .register_type::<CurrentTile>()
        .register_type::<CharacterControl>()
        .register_type::<Health>()
        .register_type::<AvoidWallsData>()
        .register_type::<ApproachAndAttackPlayerData>()
        .add_plugins({
            let plugins = DefaultPlugins.set(LogPlugin {
                filter: "wgpu_core=warn,wgpu_hal=warn".into(),
                level: log::Level::INFO,
                ..Default::default()
            });
            if fullscreen {
                plugins.set(WindowPlugin {
                    primary_window: Some(fullscreen::window()),
                    ..Default::default()
                })
            } else {
                plugins
            }
        })
        // .add_plugins(SkeinPlugin::default())
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(PhysicsDebugPlugin)
        .add_plugins(fx::FxPlugin)
        .insert_gizmo_config(
            PhysicsGizmos::default(),
            GizmoConfig {
                enabled: false,
                ..Default::default()
            },
        )
        // Lit by lamps and flashlights rather than ambient light (`lights_systems`), so
        // unlit ground has to be allowed to go dark, and coloured lights to tint.
        //.add_plugins(FlatShaderPlugin::global_with(
        //     FlatShaderBuilder::default()
        //         .dark_level(0.25)
        //         .light_tint(0.8)
        //         .build(),
        //))
        // .add_plugins(PixelShaderPlugin::default())
        .add_plugins(GamePlugin);

    if fullscreen {
        app.add_plugins(fullscreen::FullscreenPlugin);
    }

    if straight_to_asset_browser {
        app.add_systems(
            Startup,
            |mut next: ResMut<NextState<game_state::GameState>>| {
                next.set(game_state::GameState::AssetBrowser);
            },
        );
    }

    if straight_to_house_editor {
        app.add_systems(
            Startup,
            |mut next: ResMut<NextState<game_state::GameState>>| {
                next.set(game_state::GameState::HouseEditor);
            },
        );
    }

    if let Some(number) = start_level {
        app.add_systems(Startup, move |mut campaign: ResMut<campaign::Campaign>| {
            campaign.go_to(number);
        });
    }

    if straight_to_game && !straight_to_editor {
        app.add_systems(
            Startup,
            |mut next: ResMut<NextState<game_state::GameState>>| {
                next.set(game_state::GameState::InGame);
            },
        );
    }

    if straight_to_editor {
        app.add_systems(
            Startup,
            |mut commands: Commands, mut next: ResMut<NextState<game_state::GameState>>| {
                commands.init_resource::<character_editor::state::EditorSession>();
                next.set(game_state::GameState::InGame);
            },
        );
    }

    app.run();
    std::process::ExitCode::SUCCESS
}
