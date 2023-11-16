use bevy::app::{App, FixedUpdate, PluginGroup, PreUpdate, Startup, Update};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use bevy::prelude::Msaa;
use bevy::time::{Fixed, Time};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_xpbd_3d::plugins::{PhysicsPlugins};
use big_brain::BigBrainPlugin;
use pathfinding::grid::Grid;
use crate::player::systems::spawn_players::spawn_players;
use camera::systems::spawn_camera::spawn_camera;
use general::events::map_events::{LoadMap, SpawnAlien, SpawnPlayer};
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::ai::systems::approach_and_attack_player_systems::{approach_and_attack_player_scorer_system, approach_player_action_system, attack_player_action_system, can_i_see_player_system};
use crate::ai::systems::avoid_walls_systems::{avoid_walls_action_system, avoid_walls_data_system, avoid_walls_scorer_system};
use crate::ai::systems::move_forward_systems::{move_forward_action_system, move_forward_scorer_system};
use crate::ai::systems::move_towards_goal_systems::{alien_reached_goal_handler, move_towards_goal_action_system, move_towards_goal_scorer_system};
use crate::camera::components::camera::CameraOffset;
use crate::camera::systems::camera_follow::camera_follow;
use crate::enemy::components::general::AlienCounter;
use crate::enemy::systems::spawn_aliens::{alien_spawner_system, spawn_aliens};
use crate::general::components::Health;
use crate::general::events::map_events::AlienReachedGoal;
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::dynamic_movement_system::dynamic_movement;
use crate::general::systems::collision_handling_system::collision_handling_system;
use crate::general::systems::kinematic_movement_system::kinematic_movement;
use crate::general::systems::lights_systems::spawn_lights;
use crate::general::systems::map_systems::{current_tile_system, load_map_one, map_loader};
use crate::general::systems::throwing_system::throwing;
use crate::player::components::general::Controller;
use crate::player::systems::keyboard_control::keyboard_control;

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;
pub(crate) mod enemy;
pub(crate) mod ai;

pub const METERS_PER_PIXEL: f64 = 16.0;

fn main() {
    App::new()
        .add_event::<LoadMap>()
        .add_event::<SpawnAlien>()
        .add_event::<AlienReachedGoal>()
        .add_event::<SpawnPlayer>()
        .register_type::<CameraOffset>()
        .register_type::<Controller>()
        .register_type::<Health>()
        .register_type::<AvoidWallsData>()
        .register_type::<ApproachAndAttackPlayerData>()
        .insert_resource(AlienCounter::new(50))
        .insert_resource(MapGraph {
            grid: Grid::new(0,0),
            goal: (0,0)
        })
        .insert_resource(Msaa::Sample4)
        .insert_resource(Time::<Fixed>::from_seconds(0.05))
        .insert_resource(Msaa::Sample4)
        .add_plugins(
            DefaultPlugins.set(
                LogPlugin {
                    filter: "wgpu_core=warn,wgpu_hal=warn".into(),
                    level: log::Level::INFO,
                }))
        .add_plugins(PhysicsPlugins::default())
        // .add_plugins(PhysicsDebugPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(BigBrainPlugin::new(PreUpdate))
        .add_systems(
            Startup,
            (
                load_map_one,
                spawn_camera,
                spawn_lights,
            ))
        .add_systems(
            Update,
            (
                current_tile_system,
                alien_spawner_system,
                map_loader,
                spawn_players,
                spawn_aliens,
                camera_follow,
                keyboard_control,
                kinematic_movement,
                dynamic_movement,
                throwing,
                collision_handling_system,
                current_tile_system,
                alien_reached_goal_handler,
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
                approach_and_attack_player_scorer_system,
                approach_player_action_system,
                attack_player_action_system,
                move_towards_goal_scorer_system,
                move_towards_goal_action_system
            ),
        )
        .run();
}
