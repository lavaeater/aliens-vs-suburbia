use std::collections::HashSet;
use belly::build::BellyPlugin;
use bevy::app::{App, FixedUpdate, PluginGroup, PreUpdate, Startup, Update};
use bevy::{DefaultPlugins, log};
use bevy::log::LogPlugin;
use bevy::prelude::Msaa;
use bevy::time::{Fixed, Time};
use bevy::utils::HashMap;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_xpbd_3d::components::RigidBody;
use bevy_xpbd_3d::plugins::PhysicsPlugins;
use big_brain::BigBrainPlugin;
use pathfinding::grid::Grid;
use crate::player::systems::spawn_players::spawn_players;
use camera::systems::spawn_camera::spawn_camera;
use general::events::map_events::{LoadMap, SpawnAlien, SpawnPlayer};
use general::systems::map_systems::{add_tile_to_map, remove_tile_from_map};
use player::systems::build_systems::build_tower_system;
use crate::ai::components::approach_and_attack_player_components::ApproachAndAttackPlayerData;
use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::ai::components::move_towards_goal_components::{AlienReachedGoal, CantFindPath};
use crate::ai::systems::approach_and_attack_player_systems::{approach_and_attack_player_scorer_system, approach_player_action_system, attack_player_action_system, can_i_see_player_system};
use crate::ai::systems::avoid_walls_systems::{avoid_walls_action_system, avoid_walls_data_system, avoid_walls_scorer_system};
use crate::ai::systems::destroy_the_map_systems::{alien_cant_find_path, destroy_the_map_action_system, destroy_the_map_scorer_system};
use crate::ai::systems::move_forward_systems::{move_forward_action_system, move_forward_scorer_system};
use crate::ai::systems::move_towards_goal_systems::{alien_reached_goal_handler, move_towards_goal_action_system, move_towards_goal_scorer_system};
use crate::camera::components::camera::CameraOffset;
use crate::camera::systems::camera_follow::camera_follow;
use crate::enemy::components::general::AlienCounter;
use crate::enemy::systems::spawn_aliens::{alien_spawner_system, spawn_aliens};
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::{CurrentTile, ModelDefinition, ModelDefinitions};
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::dynamic_movement_system::dynamic_movement;
use crate::general::systems::collision_handling_system::collision_handling_system;
use crate::general::systems::health_monitor_system::health_monitor_system;
use crate::general::systems::kinematic_movement_system::kinematic_movement;
use crate::general::systems::lights_systems::spawn_lights;
use crate::general::systems::map_systems::{load_map_one, map_loader, TileDefinitions, update_current_tile_system};
use crate::general::systems::throwing_system::throwing;
use crate::player::components::general::Controller;
use crate::player::events::building_events::{AddTile, ChangeBuildIndicator, EnterBuildMode, ExecuteBuild, ExitBuildMode, RemoveTile};
use crate::player::systems::build_systems::{building_mode, change_build_indicator, enter_build_mode, execute_build, exit_build_mode};
use crate::player::systems::keyboard_control::input_control;
use crate::towers::events::BuildTower;
use crate::towers::systems::{alien_in_range_scorer_system, shoot_alien_system};
use crate::ui::spawn_ui::{add_health_bar, AddHealthBar, fellow_system, spawn_ui};

pub(crate) mod player;
pub(crate) mod general;
pub(crate) mod camera;
pub(crate) mod enemy;
pub(crate) mod ai;
pub(crate) mod towers;
pub(crate) mod ui;

pub const METERS_PER_PIXEL: f64 = 16.0;

fn main() {
    App::new()
        .add_event::<LoadMap>()
        .add_event::<SpawnAlien>()
        .add_event::<AlienReachedGoal>()
        .add_event::<CantFindPath>()
        .add_event::<SpawnPlayer>()
        .add_event::<EnterBuildMode>()
        .add_event::<ExitBuildMode>()
        .add_event::<ExecuteBuild>()
        .add_event::<ChangeBuildIndicator>()
        .add_event::<RemoveTile>()
        .add_event::<AddTile>()
        .add_event::<BuildTower>()
        .add_event::<AddHealthBar>()
        .register_type::<CameraOffset>()
        .register_type::<CurrentTile>()
        .register_type::<Controller>()
        .register_type::<Health>()
        .register_type::<AvoidWallsData>()
        .register_type::<ApproachAndAttackPlayerData>()
        .insert_resource(
            ModelDefinitions {
                definitions: HashMap::from(
                    [
                        ("wall", ModelDefinition {
                            name: "wall",
                            file: "map/wall_small.glb#Scene0",
                            width: 16.0,
                            height: 19.0,
                            depth: 1.0,
                            rigid_body: RigidBody::Static,
                            group: vec![CollisionLayer::Impassable],
                            mask: vec![CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player],
                        }),
                        ("floor", ModelDefinition {
                            name: "floor",
                            file: "map/floor_small.glb#Scene0",
                            width: 16.0,
                            height: 1.0,
                            depth: 16.0,
                            rigid_body: RigidBody::Static,
                            group: vec![CollisionLayer::Floor],
                            mask: vec![CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player],
                        }),
                        ("obstacle", ModelDefinition {
                            name: "obstacle",
                            file: "map/obstacle.glb#Scene0",
                            width: 16.0,
                            height: 4.0,
                            depth: 16.0,
                            rigid_body: RigidBody::Kinematic,
                            group: vec![CollisionLayer::Impassable],
                            mask: vec![CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player],
                        }),
                        ("tower", ModelDefinition {
                            name: "tower",
                            file: "map/tower_balls.glb#Scene0",
                            width: 16.0,
                            height: 8.0,
                            depth: 16.0,
                            rigid_body: RigidBody::Kinematic,
                            group: vec![CollisionLayer::Impassable],
                            mask: vec![CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player],
                        }),
                    ]),
                build_indicators: vec!["obstacle", "tower"],

            }
        )
        .insert_resource(
            TileDefinitions::new(1.0,
                                 32.0,
                                 9.5,
                                 1.0,
                                 "map/wall_small.glb#Scene0".into(),
                                 "map/floor_small.glb#Scene0".into(),
                                 "map/obstacle.glb#Scene0".into()))
        .insert_resource(AlienCounter::new(50))
        .insert_resource(MapGraph {
            path_finding_grid: Grid::new(0, 0),
            occupied_tiles: HashSet::new(),
            goal: (0, 0),
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
        .add_plugins(BellyPlugin)
        .add_systems(
            Startup,
            (
                load_map_one,
                spawn_camera,
                spawn_lights,
                spawn_ui,
            ))
        .add_systems(
            Update,
            (
                update_current_tile_system,
                alien_spawner_system,
                map_loader,
                spawn_players,
                spawn_aliens,
                camera_follow,
                input_control,
                kinematic_movement,
                dynamic_movement,
                throwing,
                collision_handling_system,
                update_current_tile_system,
                alien_reached_goal_handler,
                enter_build_mode,
                exit_build_mode,
                building_mode,
                execute_build,
                remove_tile_from_map,
                add_tile_to_map,
                change_build_indicator,
            ))
        .add_systems(
            Update,
            (
                build_tower_system,
                shoot_alien_system,
                alien_cant_find_path,
                health_monitor_system,
                add_health_bar,
                fellow_system,
            ),
        )
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
                move_towards_goal_action_system,
                alien_in_range_scorer_system,
                destroy_the_map_scorer_system,
                destroy_the_map_action_system
            ),
        )
        .run();
}
