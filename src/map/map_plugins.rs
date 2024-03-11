use bevy::app::{App, Plugin, Startup, Update};
use bevy::utils::HashMap;
use bevy_xpbd_3d::components::{LayerMask, RigidBody};
use pathfinding::grid::Grid;
use std::collections::HashSet;
use bevy::prelude::{in_state, IntoSystemConfigs, OnEnter};
use crate::alien::components::general::AlienCounter;
use crate::game_state::GameState;
use crate::general::components::CollisionLayer;
use crate::general::components::map_components::{ModelDefinition, MapModelDefinitions};
use crate::general::events::map_events::{LoadMap, SpawnAlien, SpawnPlayer};
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::map_systems::{load_map_one, map_loader, TileDefinitions, update_current_tile_system};

pub struct NonStateMapStuff;

impl Plugin for NonStateMapStuff {
    fn build(&self, app: &mut App) {
        app
            .add_event::<LoadMap>()
            .add_event::<SpawnPlayer>()
            .add_event::<SpawnAlien>()
            .insert_resource(
                MapModelDefinitions {
                    definitions: HashMap::from(
                        [
                            ("wall", ModelDefinition {
                                name: "wall",
                                file: "map/wall_small.glb#Scene0",
                                width: 16.0,
                                height: 19.0,
                                depth: 1.0,
                                rigid_body: RigidBody::Static,
                                group: LayerMask::from([CollisionLayer::Impassable]),
                                mask: LayerMask::from([CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                            }),
                            ("floor", ModelDefinition {
                                name: "floor",
                                file: "map/floor_small.glb#Scene0",
                                width: 16.0,
                                height: 1.0,
                                depth: 16.0,
                                rigid_body: RigidBody::Static,
                                group: LayerMask::from([CollisionLayer::Floor]),
                                mask: LayerMask::from([CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                            }),
                            ("obstacle", ModelDefinition {
                                name: "obstacle",
                                file: "map/obstacle.glb#Scene0",
                                width: 16.0,
                                height: 4.0,
                                depth: 16.0,
                                rigid_body: RigidBody::Kinematic,
                                group: LayerMask::from([CollisionLayer::Impassable]),
                                mask: LayerMask::from([CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                            }),
                            ("tower", ModelDefinition {
                                name: "tower",
                                file: "map/tower_balls.glb#Scene0",
                                width: 16.0,
                                height: 8.0,
                                depth: 16.0,
                                rigid_body: RigidBody::Kinematic,
                                group: LayerMask::from([CollisionLayer::Impassable]),
                                mask: LayerMask::from([CollisionLayer::Ball, CollisionLayer::Alien, CollisionLayer::Player]),
                            }),
                        ]),
                    build_indicators: vec!["obstacle", "tower"],

                }
            ).insert_resource(
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
            });
    }
}

pub struct StatefulMapPlugin;

impl Plugin for StatefulMapPlugin {
    //
    fn build(&self, app: &mut App) {
        app
            .add_plugins(NonStateMapStuff)
            .add_systems(OnEnter(GameState::InGame), load_map_one)
            .add_systems(
                Update, (
                    update_current_tile_system,
                    map_loader //Will this read the event from the load_map_one system?
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(NonStateMapStuff)
            .add_systems(
                Startup,
                (
                    load_map_one,
                ),
            )
            .add_systems(Update, (
                update_current_tile_system,
                map_loader, ),
            );
    }
}
