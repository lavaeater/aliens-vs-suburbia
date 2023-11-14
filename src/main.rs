use bevy::app::{App, FixedUpdate, PluginGroup, PreUpdate, Startup, Update};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use bevy::prelude::{Mesh, Msaa};
use bevy::scene::Scene;
use bevy::time::{Fixed, Time};
use bevy::utils::{HashMap};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_xpbd_3d::plugins::{PhysicsPlugins};
use big_brain::{BigBrainPlugin};
use crate::player::systems::spawn_players::spawn_players;
use camera::systems::spawn_camera::spawn_camera;
use crate::ai::systems::approach_player_systems::{approach_player_action_system, approach_player_scorer_system, can_i_see_player_system};
use crate::ai::systems::avoid_walls_systems::{avoid_walls_action_system, avoid_walls_data_system, avoid_walls_scorer_system};
use crate::ai::systems::move_forward_systems::{move_forward_action_system, move_forward_scorer_system};
use crate::camera::components::camera::CameraOffset;
use crate::camera::systems::camera_follow::camera_follow;
use crate::enemy::systems::spawn_aliens::spawn_aliens;
use crate::general::systems::dynamic_movement::dynamic_movement;
use crate::general::systems::kill_the_balls::kill_the_balls;
use crate::general::systems::kinematic_movement::kinematic_movement;
use crate::general::systems::lights::spawn_lights;
use crate::general::systems::load_models::Handles;
use crate::general::systems::map::spawn_map;
use crate::general::systems::throwing::throwing;
use crate::player::systems::keyboard_control::keyboard_control;

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;
pub(crate) mod enemy;
pub(crate) mod ai;

pub const METERS_PER_PIXEL: f64 = 16.0;

fn main() {
    App::new()
        .register_type::<CameraOffset>()
        .insert_resource(Msaa::Sample4)
        .insert_resource(Handles::<Mesh> {
            handles: HashMap::new()
        })
        .insert_resource(Handles::<Scene> {
            handles: HashMap::new()
        })
        .insert_resource(Time::<Fixed>::from_seconds(0.05))
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins.set(LogPlugin {
            // Use `RUST_LOG=big_brain=trace,thirst=trace cargo run --example
            // thirst --features=trace` to see extra tracing output.
            filter: "wgpu_core=warn,wgpu_hal=warn".into(),
            level: log::Level::DEBUG,
        }))
        .add_plugins(PhysicsPlugins::default())
        // .add_plugins(PhysicsDebugPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(BigBrainPlugin::new(PreUpdate))
        .add_systems(
            Startup,
            (
                spawn_map,
                spawn_players,
                spawn_aliens,
                spawn_camera,
                spawn_lights,
            ))
        .add_systems(
            Update,
            (
                camera_follow,
                keyboard_control,
                kinematic_movement,
                dynamic_movement,
                throwing,
                kill_the_balls,
            ))
        .add_systems(
            FixedUpdate,
            (
                avoid_walls_data_system,
                can_i_see_player_system,
            ))
        .add_systems(
            PreUpdate,
            (
                avoid_walls_scorer_system,
                avoid_walls_action_system,
                move_forward_scorer_system,
                move_forward_action_system,
                approach_player_scorer_system,
                approach_player_action_system
            ),
        )
        .run();
}
